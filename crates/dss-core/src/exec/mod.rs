//! The command executive: a port of `Executive.pas` / `ExecCommands.pas` /
//! `ExecOptions.pas` / `ExecHelper.pas`, Phase 3 subset. The full Pascal
//! command and option name lists are registered (so abbreviation ownership
//! matches the oracle), but only the ★-slice verbs do real work — `New`
//! (including `New circuit.`), `Edit`, `~`/`More`/`M`, `Set`/`Get`, `Solve`,
//! `CalcVoltageBases`, `Redirect`/`Compile`, `Clear`, `BuildY`, `Init`, and
//! `?`. Everything else records a clear not-ported message.
//!
//! [`Dss`] is the Rust form of `TDSSContext`: it owns the class registry,
//! the active circuit, the parsers, the enum table, and the error log.
//!
//! Split into submodules (no behavioral change): the command/option name
//! tables and dispatch ordinals live in [`tables`]; `Dss::new`'s class
//! registry in [`construct`]; the `Set`/`Get` option commands in [`set_cmd`]
//! and [`get_cmd`]. This module keeps the imports, the [`Dss`] struct and its
//! read-only accessors.

#[cfg(test)]
mod tests;

pub(crate) use std::collections::HashMap;
pub(crate) use std::path::{Path, PathBuf};

pub(crate) use dss_parser::{Parser, ParserVars};

pub(crate) use crate::circuit::{Circuit, ElemKind};
pub(crate) use crate::elements::control::{
    cap_control, espvl_control, exp_control, gen_dispatcher, inv_control, recloser, reg_control,
    relay, storage_controller, swt_control, upfc_control,
};
pub(crate) use crate::elements::general::{
    conductor_data, dynamic_exp, growth_shape, line_code, line_geometry, line_spacing, load_shape,
    price_shape, spectrum, tcc_curve, temp_shape, xfmr_code, xy_curve,
};
pub(crate) use crate::elements::meter::energymeter;
pub(crate) use crate::elements::meter::monitor;
pub(crate) use crate::elements::meter::sensor;
pub(crate) use crate::elements::pc::{
    generator, gic_line, gic_source, ind_mach012, isource, load, pvsystem, storage, upfc, vccs,
    vs_converter, vsource, windgen,
};
pub(crate) use crate::elements::pd::{
    auto_trans, capacitor, fault, fuse, gic_transformer, line, reactor, transformer,
};
pub(crate) use crate::elements::traits::{CktElement, ElemId, ElemStore, TypedStore};
pub(crate) use crate::obj::arena::ClassArena;
pub(crate) use crate::obj::base::DssObject;
pub(crate) use crate::obj::dss_enum::{EnumId, EnumRegistry};
pub(crate) use crate::obj::props::{ClassProps, ForeignClassesView, PropEngine, PropType};
pub(crate) use crate::solution::{SolveEnv, SolveMode, set_voltage_bases, solve};
pub(crate) use crate::support::command_list::CommandList;
pub(crate) use crate::util::{float_to_str, interpret_yes_no, parse_object_class_and_name};

mod auto_add;
mod batchedit;
mod command;
mod construct;
mod diakoptics;
mod distribute;
mod get_cmd;
mod helpers;
mod json_import;
mod make_pos_seq;
mod plot;
mod pstcalc;
mod reconductor;
mod reduce;
pub(crate) mod registry;
mod report;
mod save_circuit;
mod set_cmd;
mod solve;
mod tables;
mod tearing;
mod tearing_save;
mod uuids_cmd;
mod view;

pub(crate) use helpers::*;
pub(crate) use registry::{ClassStore, DssClass, ForeignClasses};
pub(crate) use tables::{EXEC_COMMANDS, EXEC_OPTIONS, PLOT_OPTIONS, cmd, opt};
pub use view::{
    ElementSnapshot, FeederSectionView, MeterReliabilityView, MeterZoneView, MonitorView,
    PdElementView, SystemYCsc,
};

/// The plot/visualize callback (`DSS.DSSPlotCallback`): given the assembled
/// `plotParams` JSON string, returns an `i32` (Pascal ignores it; kept for
/// signature parity). Boxed so a GUI consumer can capture state. `+ Send` is
/// the P7 thread-readiness rider (DE_PASCALIZE R1): it keeps `Dss: Send` (see
/// `lib.rs` `assert_send::<Dss>()`) — a GUI hook captured for a threaded engine
/// is `Send` in practice.
type PlotCallback = Box<dyn FnMut(&str) -> i32 + Send>;

/// The DSS engine context (`TDSSContext`).
pub struct Dss {
    /// The registered classes, each owning its objects in a typed
    /// [`ClassArena`] (`class.arena`) — DE_PASCALIZE R1 ownership flip off the
    /// pre-R1 `Vec<Box<dyn DssObject>>` (`PORTING_PLAN §2.1`).
    classes: Vec<DssClass>,
    /// Lowercased class name → index (Pascal `ClassNames`).
    class_by_name: HashMap<String, usize>,
    commands: CommandList,
    option_list: CommandList,
    /// `DSS.DSSExecutive.ExportCommands` — the `Export` report keyword list
    /// (Pascal `ExportOptions.DefineOptions`), abbreviation-matched (WP8.1).
    export_commands: CommandList,
    /// `DSS.DSSExecutive.ShowCommands` — the `Show` report keyword list
    /// (Pascal `ShowOptions.DefineOptions`), abbreviation-matched (WP8.1).
    show_commands: CommandList,
    /// `DSS.DSSExecutive.PlotCommands` — the `Plot` option keyword list
    /// (Pascal `PlotOptions.DefineOptions`), abbreviation-matched (WPG.17).
    plot_commands: CommandList,
    /// Main parser driving the command/edit loop (`DSS.Parser`).
    parser: Parser,
    /// Scratch parser for property values (`DSS.AuxParser`/`PropParser`).
    aux_parser: Parser,
    vars: ParserVars,
    enums: EnumRegistry,
    /// Accumulated `DoSimpleMsg` log (record-and-continue diagnostics).
    errors: crate::diag::ErrorLog,
    active_class: Option<usize>,
    /// `DSS.ActiveCircuit.ActiveCktElement` as `(class_idx, obj_idx)` — the circuit
    /// element the `Select` command (Pascal `DoSelectCmd`) made active, read by the
    /// active-element reports (`Show Yprim`). `None` until a `Select` of a circuit
    /// element runs.
    active_ckt_element: Option<(usize, usize)>,
    /// `DSS.GlobalResult`: the value returned by `?` queries and `Get`.
    last_result: String,
    /// The active circuit (`DSS.ActiveCircuit`; `MaxCircuits = 1`).
    circuit: Option<Circuit>,
    /// `DSS.DefaultBaseFreq` (`Set DefaultBaseFrequency=`).
    default_base_freq: f64,
    /// `DSS.DefaultEarthModel` (`Set EarthModel=`); default `DERI` (3). Copied
    /// into each `TLineObj.FEarthModel` at creation (Pascal `Line.pas:998`).
    default_earth_model: i32,
    /// `DSS.MaxAllocationIterations` (`Set NumAllocIterations=`); default 2.
    max_allocation_iterations: i32,
    /// `DSS.AutoShowExport` (`Set ShowExport=`); default FALSE
    /// (`DSSClass.pas:1278`). Its only *physical* upstream consumer is the
    /// `FireOffEditor` auto-open after an export (`ExportOptions.pas:637`) —
    /// a GUI no-op headless, so the flag is otherwise stored for `Set`/`Get`
    /// parity. The one engine-side reader is `DoEstimateCmd`'s `if not
    /// AutoShowExport then ParseCommand('Set showexport=yes')`
    /// (`ExecHelper.pas:4223`), ported in [`Dss::do_estimate_cmd`].
    auto_show_export: bool,
    /// `NoFormsAllowed` (`ExecOptions.pas:778`, `Set/Get AllowForms`); default
    /// TRUE (headless = no console forms). A GUI gate with no headless effect —
    /// stored so `Set AllowForms=` / `Get AllowForms` round-trip like capi015,
    /// nothing reads it (mirrors `auto_show_export`).
    no_forms_allowed: bool,
    /// `NoProgressBarFormAllowed` (`ExecOptions.pas:780`, `Set/Get
    /// AllowProgressBar`); default TRUE (headless). Stored for `Set`/`Get`
    /// parity, unread.
    no_progress_bar_form_allowed: bool,
    /// `DSS.CurrentDSSDir`: base for resolving relative script paths.
    current_dir: PathBuf,
    /// `DSS.OutputDirectory`: where reports are written (Pascal
    /// `GetOutputDirectory`). Defaults to the startup cwd; changed by
    /// `Set DataPath=` **and by `Compile`** (Pascal `DoRedirect` runs
    /// `SetDataPath(DSS, CurrDir)` for Compile before and after processing the
    /// deck — `ExecHelper.pas:546/651` — so default-named exports land next to
    /// the compiled deck; oracle-verified). Plain `Redirect` moves only
    /// `current_dir`. The non-writable-dir scratch fallback is NOT_PORTED —
    /// environment-dependent, see `set_cmd::apply_data_path`'s doc.
    output_directory: PathBuf,
    /// `DSS.LastResultFile` / `@lastfile` (Pascal `SetLastResultFile`): the path
    /// of the most recently written report (`Export`/`Show`/`Save`). Surfaced via
    /// [`Dss::last_result_file`] so the golden harness can read the produced file.
    last_result_file: String,
    /// `DSS.In_Redirect` / `DSS.Redirect_Abort`.
    in_redirect: bool,
    redirect_abort: bool,
    /// P5b diagnostic-source origin for the command line currently executing —
    /// the `NamedSource` name a span-carrying diagnostic renders. `"<command>"`
    /// for interactive/API commands; `"<file>:<line-no>"` while `do_redirect`
    /// drives a `Redirect`/`Compile` deck (scope rule v1: one command line = one
    /// source). Restored around each redirect so nested decks report correctly.
    cmd_origin: String,
    /// `DSS.CIMExporter`: the persistent hashed-UUID list state (WP8.6 step 6;
    /// GAPS_PLAN WPG.18 adds the CIM XML exporters on top).
    cim: crate::cim::CimExporter,
    /// `DSS.DSSObjs`: every general (`DSS_OBJECT`) object in global creation
    /// order — the list the whole-circuit `Dump` walks after `CktElements`
    /// (Pascal `ExecHelper.pas:1373`; populated at `AddObject`, `:1899`).
    dss_objs: Vec<ElemId>,
    /// `DSS.DaisySize` (`DSSClass.pas:741`, default 1.0; `Set Daisysize=`):
    /// a GUI daisy-plot marker radius written into the plot-callback payload.
    /// Lives on the DSS context, not the circuit.
    daisy_size: f64,
    /// The A-Diakoptics child engines (Pascal `ActiveCircuit[2..NumOfActors]`),
    /// owned by the coordinator per plan D3 (`children: Vec<Dss>`, sequential —
    /// no threads, no shared state). Empty until `set ADiakoptics=yes` runs
    /// `ADiakopticsInit`; `child[0]` is Pascal actor 2 (the feeder-head zone 1),
    /// `child[k-2]` is actor `k`. Rebuilt from scratch each init (cleared then
    /// repopulated in state 2, `diakoptics/engine.rs`); `set ADiakoptics=no`
    /// (flag-only, per §WP-AD.3) and `Clear` leave the vector intact — the stale
    /// children are inert (they are only read while `Solution.ADiakoptics` is
    /// true, which a re-init re-establishes after clearing them).
    ad_children: Vec<Dss>,
    /// `DSS.DSSPlotCallback` (`Common/DSSClass.pas:658`). Native replacement for
    /// the C export `DSS_RegisterPlotCallback` (`CAPI_DSS.pas:267`). `None` =>
    /// `Plot` is a total no-op and `DoVisualizeCmd` skips its JSON emission —
    /// byte-identical to the pinned headless oracle. Registering it is the single
    /// gate that subsumes BOTH Pascal gates (`NoFormsAllowed=False` AND
    /// `DSSPlotCallback<>NIL`); see [`Dss::register_plot_callback`].
    plot_callback: Option<PlotCallback>,
}

impl Dss {
    /// Accumulated diagnostics (`DoSimpleMsg`/`DoErrorMsg` log). The stable
    /// identity of each is its [`crate::diag::DssDiagnostic::code`], not its
    /// text (P5 golden policy).
    pub fn errors(&self) -> &[crate::diag::DssDiagnostic] {
        &self.errors
    }

    /// The message texts of the recorded diagnostics — convenience for harness
    /// callers that compare on wording (`Dss::errors` carries the structured
    /// diagnostics; key tests on `code`, not text).
    pub fn error_texts(&self) -> Vec<String> {
        self.errors.texts()
    }

    /// Register the plot/visualize callback — the native Rust replacement for
    /// the C export `DSS_RegisterPlotCallback` (`CAPI_DSS.pas:267`).
    ///
    /// The closure receives the assembled `plotParams` JSON string (the exact
    /// payload Pascal builds with `TJSONObject.FormatJSON` and hands to
    /// `DSS.DSSPlotCallback`) and returns an `i32` (Pascal ignores the return;
    /// kept for signature parity). A registered callback is the single opt-in
    /// gate: it collapses Pascal's two guards (`not NoFormsAllowed` AND
    /// `DSSPlotCallback<>NIL`) into one, which is the faithful headless
    /// equivalent — the console-only `NoFormsAllowed`/`AllowForms`/error-5096
    /// machinery models a text terminal a headless library does not have and
    /// stays NOT_PORTED. With no callback, `Plot` is a total no-op and
    /// `Visualize` runs its guards but emits no JSON, exactly like the pinned
    /// oracle.
    pub fn register_plot_callback(&mut self, cb: impl FnMut(&str) -> i32 + Send + 'static) {
        self.plot_callback = Some(Box::new(cb));
    }

    /// Drop the plot callback (maps `DSS_RegisterPlotCallback(NULL)` /
    /// dss-python `plot.disable()`): `Plot` reverts to a total no-op.
    pub fn unregister_plot_callback(&mut self) {
        self.plot_callback = None;
    }

    /// The most recent query/`Get` result (`DSS.GlobalResult`).
    pub fn result(&self) -> &str {
        &self.last_result
    }

    /// The registered class names, in `construct.rs` registration order — the
    /// oracle the typed-arena ordering test (`obj::arena::tests`) compares
    /// `ElemId::CLASS_NAMES` and the `ClassArena` layout against.
    #[cfg(test)]
    pub(crate) fn registered_class_names(&self) -> Vec<&'static str> {
        self.classes.iter().map(|c| c.props.class_name()).collect()
    }

    /// The **live** per-`DssClass` arena's class name at each registration slot
    /// (`c.arena.class_name()`, the actual runtime storage) — the load-bearing
    /// twin of [`Self::registered_class_names`] for the ordering test: it proves
    /// `ClassArena::empty_for` selected the correct variant for every class in
    /// registration order, not just that the standalone `Elements` aggregate does.
    #[cfg(test)]
    pub(crate) fn live_arena_class_names(&self) -> Vec<&'static str> {
        self.classes.iter().map(|c| c.arena.class_name()).collect()
    }

    /// Per registered class, whether `exec/construct.rs` registered it as a
    /// **circuit-element** class (`DssClass::ckt_class`, which carries an
    /// `ElemKind`) rather than a plain `DSS_OBJECT` (`dss_object`) — the
    /// independent oracle for the arena's `ckt`/`data` tag column
    /// (`obj::arena::tests::arena_tag_matches_registry_ckt_classes`).
    #[cfg(test)]
    pub(crate) fn registered_class_is_ckt(&self) -> Vec<bool> {
        self.classes.iter().map(|c| c.kind.is_some()).collect()
    }

    /// The active circuit, if `New circuit.` has run.
    pub fn circuit(&self) -> Option<&Circuit> {
        self.circuit.as_ref()
    }

    pub fn circuit_mut(&mut self) -> Option<&mut Circuit> {
        self.circuit.as_mut()
    }

    /// The path of the most recently written report (`DSS.LastResultFile`),
    /// empty until the first `Export`/`Show`/`Save` writes a file.
    pub fn last_result_file(&self) -> &str {
        &self.last_result_file
    }

    /// The path of the most recently written `Show` report (the `@lastshowfile`
    /// parser var; Pascal's `ShowResults` procedures set only this, not
    /// `GlobalResult`/`@lastfile`). Empty until the first `Show` writes a file.
    pub fn last_show_file(&self) -> &str {
        self.vars.get("@lastshowfile").unwrap_or("")
    }

    /// Read-only view of the registered class list (`DSS.DSSClassList`), for
    /// the report formatters' coverage unit tests.
    #[cfg(test)]
    pub(crate) fn registered_classes(&self) -> &[DssClass] {
        &self.classes
    }

    /// Mutable view of the registered class list, for unit tests that must
    /// drive an object through seams the property surface does not expose yet
    /// (RP3.7 A1: the per-phase `InterpretSwitchState` mechanics land on the
    /// arena object before A2 wires them into the property parser; the test
    /// then applies the deferred `RefAction`s exactly like the executive).
    #[cfg(test)]
    pub(crate) fn registered_classes_mut(&mut self) -> &mut [DssClass] {
        &mut self.classes
    }
}

impl Default for Dss {
    fn default() -> Self {
        Self::new()
    }
}

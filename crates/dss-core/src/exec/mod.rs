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
    cap_control, gen_dispatcher, reg_control, storage_controller, swt_control,
};
pub(crate) use crate::elements::general::{
    conductor_data, growth_shape, line_code, line_geometry, line_spacing, load_shape, price_shape,
    spectrum, tcc_curve, temp_shape, xfmr_code, xy_curve,
};
pub(crate) use crate::elements::meter::energymeter;
pub(crate) use crate::elements::meter::monitor;
pub(crate) use crate::elements::meter::sensor;
pub(crate) use crate::elements::pc::{generator, load, vsource};
pub(crate) use crate::elements::pd::{capacitor, fault, fuse, line, reactor, transformer};
pub(crate) use crate::elements::traits::{CktElement, ElemRef, ElemStore};
pub(crate) use crate::obj::base::DssObject;
pub(crate) use crate::obj::dss_enum::{EnumId, EnumRegistry};
pub(crate) use crate::obj::props::{ClassProps, ForeignClassesView, PropEngine, PropType};
pub(crate) use crate::solution::{SolveEnv, SolveMode, set_voltage_bases, solve};
pub(crate) use crate::support::command_list::CommandList;
pub(crate) use crate::util::{float_to_str, interpret_yes_no, parse_object_class_and_name};

mod command;
mod construct;
mod get_cmd;
mod helpers;
mod registry;
mod set_cmd;
mod solve;
mod tables;
mod view;

pub(crate) use helpers::*;
pub(crate) use registry::{ClassStore, DssClass, ForeignClasses};
pub(crate) use tables::{EXEC_COMMANDS, EXEC_OPTIONS, cmd, opt};
pub use view::{ElementSnapshot, MeterZoneView, MonitorView, SystemYCsc};

/// The DSS engine context (`TDSSContext`).
pub struct Dss {
    classes: Vec<DssClass>,
    /// Lowercased class name → index (Pascal `ClassNames`).
    class_by_name: HashMap<String, usize>,
    commands: CommandList,
    option_list: CommandList,
    /// Main parser driving the command/edit loop (`DSS.Parser`).
    parser: Parser,
    /// Scratch parser for property values (`DSS.AuxParser`/`PropParser`).
    aux_parser: Parser,
    vars: ParserVars,
    enums: EnumRegistry,
    /// Accumulated `DoSimpleMsg` log (record-and-continue errors).
    errors: Vec<String>,
    active_class: Option<usize>,
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
    /// `DSS.CurrentDSSDir`: base for resolving relative script paths.
    current_dir: PathBuf,
    /// `DSS.In_Redirect` / `DSS.Redirect_Abort`.
    in_redirect: bool,
    redirect_abort: bool,
}

impl Dss {
    /// Accumulated error messages (`DoSimpleMsg` log).
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    /// The most recent query/`Get` result (`DSS.GlobalResult`).
    pub fn result(&self) -> &str {
        &self.last_result
    }

    /// The active circuit, if `New circuit.` has run.
    pub fn circuit(&self) -> Option<&Circuit> {
        self.circuit.as_ref()
    }

    pub fn circuit_mut(&mut self) -> Option<&mut Circuit> {
        self.circuit.as_mut()
    }
}

impl Default for Dss {
    fn default() -> Self {
        Self::new()
    }
}

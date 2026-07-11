//! The per-class property table ([`ClassProps`]) and the generic
//! parse/edit/get engine that drives the typed [`DssObject`] accessors — the
//! Rust replacement for Pascal's `DSSObjectHelper.ParseObjPropertyValue` /
//! `GetObjPropertyValue`. Property indices are 1-based throughout, matching the
//! Pascal `TProp` ordinals.
//!
//! Split by concern (no behavioral change): the table type, its constructor, the
//! name/index accessors and the `edit_property` loop body live here; the value
//! parser (`parse_into`) is in [`parse`] and the `?`/`DumpProperties` renderer
//! (`get_value`) in [`value`].

mod parse;
mod typed;
mod value;

use crate::obj::base::DssObject;
use crate::support::command_list::CommandList;
use dss_parser::ParserError;

use super::{PropDef, PropEngine, PropType};

/// A class's property table plus its abbreviation-matching command list — the
/// Rust stand-in for the per-`TDSSClass` `PropertyType[]`/`CommandList` state.
///
/// Properties are 1-based; slot 0 is an unused placeholder so `props[idx]`
/// lines up with the Pascal ordinals. The common `Like` (`MakeLikeProperty`) is
/// appended automatically, mirroring `inherited DefineProperties`.
#[derive(Debug)]
pub struct ClassProps {
    class_name: &'static str,
    props: Vec<PropDef>,
    command_list: CommandList,
}

impl ClassProps {
    /// Build from the class-specific property rows (1-based order, excluding
    /// `Like`). `abbrev` mirrors `CommandList.Abbrev` — `GrowthShape` is the one
    /// class that disables it.
    pub fn new(class_name: &'static str, mut defs: Vec<PropDef>, abbrev: bool) -> Self {
        defs.push(PropDef::make_like("Like"));
        let names: Vec<String> = defs.iter().map(|d| d.name.to_string()).collect();
        let mut command_list = CommandList::new(names);
        command_list.abbrev_allowed = abbrev;

        let mut props = Vec::with_capacity(defs.len() + 1);
        props.push(PropDef::base("", PropType::Integer)); // slot 0, never addressed
        props.extend(defs);

        Self {
            class_name,
            props,
            command_list,
        }
    }

    pub fn class_name(&self) -> &'static str {
        self.class_name
    }

    pub fn num_properties(&self) -> usize {
        self.props.len() - 1
    }

    pub fn prop(&self, idx: usize) -> &PropDef {
        &self.props[idx]
    }

    pub fn property_name(&self, idx: usize) -> &'static str {
        self.props[idx].name
    }

    /// Pascal `PropertyIndex`/`CommandList.GetCommand`: resolve a (possibly
    /// abbreviated) property name to its 1-based index.
    pub fn property_index(&self, name: &str) -> Option<usize> {
        self.command_list.get_command(name).map(|i| i + 1)
    }

    /// One iteration of the Pascal `Edit` loop body: parse + write, record the
    /// set order (`SetAsNextSeq`), then run `PropertySideEffects`. A
    /// number-conversion failure aborts before any of the bookkeeping, exactly
    /// as the Pascal exception would unwind past `SetAsNextSeq`.
    pub fn edit_property(
        &self,
        obj: &mut dyn DssObject,
        idx: usize,
        value: &str,
        eng: &mut PropEngine,
    ) -> Result<(), ParserError> {
        let prev_int = self.parse_into(obj, idx, value, eng)?;
        obj.data_mut().set_as_next_seq(idx);
        obj.side_effects(idx, prev_int);
        Ok(())
    }
}

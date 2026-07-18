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

mod json;
mod json_set;
#[cfg(test)]
mod json_tests;
mod parse;
mod typed;
mod value;

use crate::obj::base::DssObject;
use crate::obj::props::PropFlags;
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
    /// Pascal `TDSSClass.AltPropertyOrder` (`DSSClass.pas:1957-2010`): the fixed
    /// property-index order the JSON reader (`FillObjFromJSON`) walks — natural
    /// (`TProp` ordinal) order with `Like` and `ORDERING_FIRST` props hoisted to
    /// the front and the action / `ORDERING_LAST` props pushed to the back, minus
    /// the redundant / suppressed / struct-index props. Precomputed once here.
    alt_property_order: Vec<usize>,
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

        let alt_property_order = compute_alt_property_order(&props);

        Self {
            class_name,
            props,
            command_list,
            alt_property_order,
        }
    }

    /// Pascal `TDSSClass.AltPropertyOrder`: the property-index sweep order the
    /// JSON reader uses. See the [`ClassProps::alt_property_order`] field.
    pub fn alt_property_order(&self) -> &[usize] {
        &self.alt_property_order
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

/// Pascal `TDSSClass.DefineProperties`'s `AltPropertyOrder` build
/// (`DSSClass.pas:1957-2010`): assign a `zorder` to every 1-based property
/// (`Like` → -1000; `ORDERING_FIRST` → an increasing block from -999; the action
/// / `ORDERING_LAST` props → an increasing block from 999; everything else keeps
/// its ordinal), sort ascending, then drop the redundant / JSON-suppressed /
/// struct-index / alt-index props. The result is the fixed order the JSON reader
/// applies present keys in. Ordinals are all distinct, so the sort is a total
/// order (no tie-break needed).
fn compute_alt_property_order(props: &[PropDef]) -> Vec<usize> {
    let n = props.len().saturating_sub(1); // props[0] is the unused slot 0
    let mut zorder = vec![0i32; n + 1];
    let mut next_start: i32 = -999;
    let mut next_end: i32 = 999;
    for (i, z) in zorder.iter_mut().enumerate().take(n + 1).skip(1) {
        let pd = &props[i];
        *z = if pd.ptype == PropType::MakeLike {
            -1000
        } else if pd.flags.contains(PropFlags::ORDERING_FIRST) {
            let v = next_start;
            next_start += 1;
            v
        } else if pd.ptype == PropType::Action || pd.flags.contains(PropFlags::ORDERING_LAST) {
            let v = next_end;
            next_end += 1;
            v
        } else {
            i as i32
        };
    }

    let mut order: Vec<usize> = (1..=n).collect();
    order.sort_by_key(|&i| zorder[i]);
    order.retain(|&i| {
        let f = props[i].flags;
        !(f.contains(PropFlags::SUPPRESS_JSON)
            || f.contains(PropFlags::ALT_INDEX)
            || f.contains(PropFlags::INTEGER_STRUCT_INDEX)
            || f.contains(PropFlags::REDUNDANT))
    });
    order
}

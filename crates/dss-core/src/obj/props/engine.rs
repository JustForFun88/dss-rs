//! The engine's shared scratch state ([`PropEngine`]) and the read view of the
//! other registered classes ([`ForeignClassesView`]) it uses to resolve
//! object references mid-edit.

use crate::elements::traits::ElemRef;
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use dss_parser::{Parser, ParserVars};

/// Shared scratch state the engine threads through, the equivalent of Pascal's
/// context-owned `PropParser`/`AuxParser`, the enum registry, and the
/// `DoSimpleMsg` sink. `parser` must be a dedicated scratch parser, not the one
/// driving the outer `Edit` loop.
pub struct PropEngine<'a> {
    pub parser: &'a mut Parser,
    pub vars: &'a ParserVars,
    pub enums: &'a EnumRegistry,
    pub errors: &'a mut crate::diag::ErrorLog,
    /// Read view of every class except the one being edited, alive for the
    /// duration of an edit so `ObjectRef` properties can resolve immediately
    /// (Pascal resolves `cls.Find` mid-`Edit`; see [`ForeignClassesView`]).
    /// `None` outside the executive's edit loop (unit tests, etc.).
    pub foreign: Option<&'a dyn ForeignClassesView<'a>>,
}

/// A read view of the other registered classes, the abstraction `parse_into`
/// uses to resolve an `ObjectRef` to a live object (Pascal `cls.Find`). The
/// executive implements it over the class registry minus the active class; the
/// returned `ElemRef` is stable (nothing is deleted except whole-circuit
/// `Clear`, PORTING_PLAN §2.1).
pub trait ForeignClassesView<'a> {
    /// Case-insensitive lookup of `name` in class `class`. `None` when the
    /// class or the object is unknown.
    fn find(&self, class: &str, name: &str) -> Option<(ElemRef, &'a dyn DssObject)>;

    /// Case-insensitive lookup of a full `Class.Name` reference (Pascal
    /// `GetCktElementIndex`). The returned `String` is the canonical
    /// `FullName` (`Class.name`) used by dumps. `None` when the value has no
    /// class prefix or nothing matches.
    fn find_full(&self, full_name: &str) -> Option<(ElemRef, &'a dyn DssObject, String)> {
        let _ = full_name;
        None
    }
}

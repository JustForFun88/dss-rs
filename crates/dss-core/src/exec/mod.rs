//! The command executive: a focused port of `Executive.pas` /
//! `ExecCommands.pas` / `ExecHelper.pas` covering the Phase 2 verbs —
//! `New`, `Edit`, `~`/`More`/`M`, `Clear`, and `?`. `Redirect`/`Compile`
//! (recursive file execution) and `Set`/`Get` (the global options registry)
//! are registered but record a clear not-implemented message until Phase 3.
//!
//! [`Dss`] is the Rust form of `TDSSContext`: it owns the class registry,
//! the parsers, the enum table, and the error log. There is no active circuit
//! yet (Phase 3), so the "you must create a circuit first" gate and the
//! `circuit`/`solution` pseudo-classes are stubbed.

use std::collections::HashMap;

use dss_parser::{Parser, ParserVars};

use crate::elements::general::{spectrum, tcc_curve};
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropEngine, PropType};
use crate::support::command_list::CommandList;
use crate::util::parse_object_class_and_name;

/// A class constructor: build a fresh, all-default object of the class.
type NewObjectFn = fn(&str) -> Box<dyn DssObject>;

/// One registered class plus its live objects — the Rust stand-in for a
/// `TDSSClass` together with its `ElementList`/`ElementNameList`.
struct DssClass {
    props: ClassProps,
    new_object: NewObjectFn,
    objects: Vec<Box<dyn DssObject>>,
    /// Lowercased object name → index (Pascal `ElementNameList`, THashList).
    name_to_idx: HashMap<String, usize>,
    /// Active object index (`ActiveElement`).
    active: Option<usize>,
}

impl DssClass {
    /// Pascal `SetActive`: make the named object active; returns whether it
    /// existed.
    fn set_active(&mut self, name: &str) -> bool {
        match self.name_to_idx.get(&name.to_lowercase()) {
            Some(&idx) => {
                self.active = Some(idx);
                true
            }
            None => false,
        }
    }
}

/// Top-level command verbs implemented so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verb {
    New,
    Edit,
    More,
    Clear,
    Query,
    Redirect,
    Compile,
    Set,
    Get,
}

/// Verb spellings in registration order; the `CommandList` is built from these
/// names and the index maps back here for the verb. `M`/`~` are aliases of
/// `More` (Pascal `Cmd.M`/`Cmd.tilde`). The relative order follows the Pascal
/// `TExecCommand` enum (ExecCommands.pas) so abbreviation ownership among the
/// implemented subset matches the oracle; full prefix fidelity (e.g. `cl` →
/// `Close`, `s` → `Select`) needs the complete command list, ported later.
const VERBS: &[(&str, Verb)] = &[
    ("New", Verb::New),
    ("Edit", Verb::Edit),
    ("More", Verb::More),
    ("M", Verb::More),
    ("~", Verb::More),
    ("Compile", Verb::Compile),
    ("Set", Verb::Set),
    ("Redirect", Verb::Redirect),
    ("?", Verb::Query),
    ("Clear", Verb::Clear),
    ("Get", Verb::Get),
];

/// The DSS engine context (`TDSSContext`).
pub struct Dss {
    classes: Vec<DssClass>,
    /// Lowercased class name → index (Pascal `ClassNames`).
    class_by_name: HashMap<String, usize>,
    commands: CommandList,
    /// Main parser driving the command/edit loop (`DSS.Parser`).
    parser: Parser,
    /// Scratch parser for property values (`DSS.AuxParser`/`PropParser`).
    aux_parser: Parser,
    vars: ParserVars,
    enums: EnumRegistry,
    /// Accumulated `DoSimpleMsg` log (record-and-continue errors).
    errors: Vec<String>,
    active_class: Option<usize>,
    /// `DSS.GlobalResult`: the value returned by the last `?` query.
    last_result: String,
}

impl Dss {
    pub fn new() -> Self {
        let enums = EnumRegistry::new();
        let commands = CommandList::new(VERBS.iter().map(|(name, _)| *name));

        // Class registry. More classes are registered here as they are ported.
        let classes = vec![
            DssClass {
                props: tcc_curve::class_props(),
                new_object: |name| Box::new(tcc_curve::TccCurveObj::new(name)),
                objects: Vec::new(),
                name_to_idx: HashMap::new(),
                active: None,
            },
            DssClass {
                props: spectrum::class_props(),
                new_object: |name| Box::new(spectrum::SpectrumObj::new(name)),
                objects: Vec::new(),
                name_to_idx: HashMap::new(),
                active: None,
            },
        ];
        let class_by_name = classes
            .iter()
            .enumerate()
            .map(|(i, c)| (c.props.class_name().to_lowercase(), i))
            .collect();

        Self {
            classes,
            class_by_name,
            commands,
            parser: Parser::new(),
            aux_parser: Parser::new(),
            vars: ParserVars::new(),
            enums,
            errors: Vec::new(),
            active_class: None,
            last_result: String::new(),
        }
    }

    /// Accumulated error messages (`DoSimpleMsg` log).
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    /// The most recent `?` query result (`DSS.GlobalResult`).
    pub fn result(&self) -> &str {
        &self.last_result
    }

    /// Process one command line (Pascal `ProcessCommand`). Errors are recorded
    /// in [`Dss::errors`] (record-and-continue), and a `?` query leaves its
    /// answer in [`Dss::result`].
    pub fn command(&mut self, cmd_line: &str) {
        self.parser.set_auto_increment(false);
        self.parser.set_cmd_string(cmd_line);
        let param_name = self.parser.next_param(&self.vars);
        let param = self.parser.make_string(&self.vars);
        if param.is_empty() {
            return; // blank line
        }

        // A command verb has no `name=` part; a `prop=value` reference does.
        let verb = if param_name.is_empty() {
            self.commands.get_command(&param).map(|i| VERBS[i].1)
        } else {
            None
        };

        match verb {
            Some(Verb::New) => self.do_new_cmd(),
            Some(Verb::Edit) => self.do_edit_cmd(),
            Some(Verb::More) => {
                self.edit_active();
            }
            Some(Verb::Clear) => self.do_clear_cmd(),
            Some(Verb::Query) => self.do_query_cmd(),
            Some(Verb::Redirect) | Some(Verb::Compile) => {
                self.errors.push(
                    "Redirect/Compile are not implemented in the Phase 2 executive".to_string(),
                );
            }
            Some(Verb::Set) | Some(Verb::Get) => {
                // The global options registry arrives in Phase 3.
                self.errors
                    .push("Set/Get are not implemented in the Phase 2 executive".to_string());
            }
            None => {
                self.errors.push(format!("Unknown Command: \"{param}\""));
            }
        }
    }

    /// Pascal `GetObjClassAndName`: read the `class.name` token (optionally
    /// prefixed `object=`) from the main parser.
    fn get_obj_class_and_name(&mut self) -> (String, String) {
        let param_name = self.parser.next_param(&self.vars).to_lowercase();
        let param = self.parser.make_string(&self.vars);
        if !param_name.is_empty() && !crate::util::compare_text_shortest_eq(&param_name, "object") {
            self.errors
                .push("object=Class.Name expected as first parameter in command.".to_string());
            return (String::new(), String::new());
        }
        parse_object_class_and_name(&mut self.parser, &self.vars, &param)
    }

    /// Pascal `DoNewCmd` → `AddObject`.
    fn do_new_cmd(&mut self) {
        let (obj_class, obj_name) = self.get_obj_class_and_name();
        if obj_class.eq_ignore_ascii_case("solution") {
            self.errors.push(
                "You cannot create new Solution objects through the command interface.".to_string(),
            );
            return;
        }
        if obj_class.eq_ignore_ascii_case("circuit") {
            // TODO(phase3): MakeNewCircuit. DSS_OBJECT classes don't need one.
            return;
        }
        self.add_object(&obj_class, &obj_name);
    }

    /// Pascal `DoEditCmd` → `EditObject`.
    fn do_edit_cmd(&mut self) {
        let (obj_class, obj_name) = self.get_obj_class_and_name();
        if obj_class.eq_ignore_ascii_case("circuit") {
            return;
        }
        let Some(&ci) = self.class_by_name.get(&obj_class.to_lowercase()) else {
            self.errors.push(format!(
                "Edit Command: Object Type \"{obj_class}\" not found."
            ));
            return;
        };
        self.active_class = Some(ci);
        if self.classes[ci].set_active(&obj_name) {
            self.edit_active();
        }
    }

    /// Pascal `AddObject` for the `DSS_OBJECT` path: create the object unless it
    /// already exists (then it just becomes active), and edit the rest of the
    /// line.
    fn add_object(&mut self, obj_class: &str, name: &str) {
        let Some(&ci) = self.class_by_name.get(&obj_class.to_lowercase()) else {
            self.errors.push(format!(
                "New Command: Object Type \"{obj_class}\" not found."
            ));
            return;
        };
        self.active_class = Some(ci);

        if name.is_empty() {
            self.errors.push("Object Name Missing".to_string());
            return;
        }

        if !self.classes[ci].set_active(name) {
            let cls = &mut self.classes[ci];
            let obj = (cls.new_object)(name);
            let idx = cls.objects.len();
            // The constructor lowercases the name (Pascal `Name := AnsiLowerCase`).
            cls.name_to_idx.insert(obj.data().name().to_string(), idx);
            cls.objects.push(obj);
            cls.active = Some(idx);
        }
        self.edit_active();
    }

    /// Pascal `DoClearCmd`: drop every object (Phase 2 has no circuit state).
    fn do_clear_cmd(&mut self) {
        for cls in &mut self.classes {
            cls.objects.clear();
            cls.name_to_idx.clear();
            cls.active = None;
        }
        self.active_class = None;
        self.errors.clear();
    }

    /// Pascal `DoQueryCmd`: `? class.obj.prop` → store the property value in
    /// [`Dss::last_result`].
    fn do_query_cmd(&mut self) {
        self.parser.next_param(&self.vars);
        let full = self.parser.make_string(&self.vars);
        let (obj_name, prop_name) = parse_obj_name(&full);

        let (class_name, name) = {
            // Reuse the class.name splitter (no @var work needed on a query).
            let mut p = Parser::new();
            parse_object_class_and_name(&mut p, &self.vars, &obj_name)
        };

        self.last_result = "Property Unknown".to_string();
        let Some(&ci) = self.class_by_name.get(&class_name.to_lowercase()) else {
            self.errors
                .push(format!("Error! Object \"{obj_name}\" not found."));
            return;
        };
        self.active_class = Some(ci);
        if !self.classes[ci].set_active(&name) {
            self.errors
                .push(format!("Error! Object \"{obj_name}\" not found."));
            return;
        }
        let cls = &self.classes[ci];
        let oi = cls.active.expect("just set active");
        if let Some(idx) = cls.props.property_index(&prop_name) {
            self.last_result = cls
                .props
                .get_value(cls.objects[oi].as_ref(), idx, &self.enums);
        }
    }

    /// The body of Pascal `TDSSClass.Edit`: iterate `name=value` parameters on
    /// the main parser against the active object, then `EndEdit`.
    fn edit_active(&mut self) {
        let Some(ci) = self.active_class else {
            self.errors
                .push("There is no active element to edit.".to_string());
            return;
        };
        let Dss {
            classes,
            parser,
            aux_parser,
            vars,
            enums,
            errors,
            ..
        } = self;
        let DssClass {
            props,
            objects,
            name_to_idx,
            active,
            ..
        } = &mut classes[ci];
        let Some(oi) = *active else {
            errors.push("There is no active element to edit.".to_string());
            return;
        };

        let mut param_pointer: i64 = 0;
        let mut param_name = parser.next_param(vars);
        let mut param = parser.make_string(vars);
        while !param.is_empty() {
            if param_name.is_empty() {
                param_pointer += 1;
            } else {
                param_pointer = props
                    .property_index(&param_name)
                    .map(|i| i as i64)
                    .unwrap_or(0);
            }

            if param_pointer <= 0 || param_pointer as usize > props.num_properties() {
                if param_name.is_empty() {
                    errors.push(format!(
                        "Unknown parameter for value \"{param}\" in object \"{}.{}\"",
                        props.class_name(),
                        objects[oi].data().name()
                    ));
                } else {
                    errors.push(format!(
                        "Unknown parameter \"{param_name}\" (value \"{param}\") for object \"{}.{}\"",
                        props.class_name(),
                        objects[oi].data().name()
                    ));
                }
            } else {
                let idx = param_pointer as usize;
                if props.prop(idx).ptype == PropType::MakeLike {
                    make_like(objects, name_to_idx, oi, &param, errors, props.class_name());
                    objects[oi].data_mut().set_as_next_seq(idx);
                    objects[oi].side_effects(idx, 0);
                } else {
                    let mut eng = PropEngine {
                        parser: aux_parser,
                        vars,
                        enums,
                        errors,
                    };
                    if let Err(e) = props.edit_property(objects[oi].as_mut(), idx, &param, &mut eng)
                    {
                        errors.push(e.message().to_string());
                    }
                }
            }

            param_name = parser.next_param(vars);
            param = parser.make_string(vars);
        }

        objects[oi].end_edit();
    }
}

impl Default for Dss {
    fn default() -> Self {
        Self::new()
    }
}

/// Pascal `MakeLikeProperty` set path: find the source object by name in the
/// same class, clone it, and copy its state onto the target.
fn make_like(
    objects: &mut [Box<dyn DssObject>],
    name_to_idx: &HashMap<String, usize>,
    target: usize,
    source_name: &str,
    errors: &mut Vec<String>,
    class_name: &str,
) {
    match name_to_idx.get(&source_name.to_lowercase()) {
        Some(&si) => {
            let src = objects[si].clone_box();
            objects[target].make_like(src.as_ref());
        }
        None => {
            errors.push(format!(
                "Error in {class_name} MakeLike: \"{source_name}\" not found."
            ));
        }
    }
}

/// Pascal `ParseObjName`: split `Class.Object.Property` into `(Class.Object,
/// Property)`. With no dot, the whole string is the property name.
fn parse_obj_name(fullname: &str) -> (String, String) {
    match fullname.find('.') {
        None => (String::new(), fullname.to_string()),
        Some(dot1) => {
            let rest = &fullname[dot1 + 1..];
            match rest.find('.') {
                None => (fullname[..dot1].to_string(), rest.to_string()),
                Some(dot2) => (
                    fullname[..dot1 + 1 + dot2].to_string(),
                    rest[dot2 + 1..].to_string(),
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(dss: &mut Dss, what: &str) -> String {
        dss.command(&format!("? {what}"));
        dss.result().to_string()
    }

    #[test]
    fn new_and_query_defaults() {
        let mut dss = Dss::new();
        dss.command("New TCC_Curve.test");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(query(&mut dss, "TCC_Curve.test.NPts"), "0");
        assert_eq!(query(&mut dss, "TCC_Curve.test.C_Array"), "");
        assert_eq!(query(&mut dss, "TCC_Curve.test.T_Array"), "");
        assert_eq!(query(&mut dss, "TCC_Curve.test.Like"), "");
    }

    #[test]
    fn new_with_inline_edits() {
        let mut dss = Dss::new();
        dss.command("New TCC_Curve.t npts=3 C_array=(1 2 3) T_array=(0.1 0.2 0.3)");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(query(&mut dss, "tcc_curve.t.npts"), "3");
        assert_eq!(query(&mut dss, "tcc_curve.t.c_array"), "[ 1 2 3]");
        assert_eq!(query(&mut dss, "tcc_curve.t.t_array"), "[ 0.1 0.2 0.3]");
    }

    #[test]
    fn edit_and_more_continue_the_object() {
        let mut dss = Dss::new();
        dss.command("New TCC_Curve.t npts=2");
        dss.command("Edit TCC_Curve.t C_array=(5 6)");
        dss.command("~ T_array=(9 8)");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(query(&mut dss, "tcc_curve.t.c_array"), "[ 5 6]");
        assert_eq!(query(&mut dss, "tcc_curve.t.t_array"), "[ 9 8]");
    }

    #[test]
    fn make_like_copies_state() {
        let mut dss = Dss::new();
        dss.command("New TCC_Curve.a npts=2 C_array=(1 2) T_array=(3 4)");
        dss.command("New TCC_Curve.b like=a");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(query(&mut dss, "tcc_curve.b.npts"), "2");
        assert_eq!(query(&mut dss, "tcc_curve.b.c_array"), "[ 1 2]");
        assert_eq!(query(&mut dss, "tcc_curve.b.t_array"), "[ 3 4]");
    }

    #[test]
    fn make_like_copies_prp_sequence() {
        // Pascal `TDSSObject.MakeLike` copies the source's PrpSequence, then
        // the Edit loop stamps the Like property itself — so a Save-order walk
        // of the target yields NPts, C_Array, T_Array, Like.
        let mut dss = Dss::new();
        dss.command("New TCC_Curve.a npts=2 C_array=(1 2) T_array=(3 4)");
        dss.command("New TCC_Curve.b like=a");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());

        let cls = &dss.classes[0];
        let oi = cls.name_to_idx["b"];
        let data = cls.objects[oi].data();
        assert_eq!(data.next_property_set(None), Some(1)); // NPts
        assert_eq!(data.next_property_set(Some(1)), Some(2)); // C_Array
        assert_eq!(data.next_property_set(Some(2)), Some(3)); // T_Array
        assert_eq!(data.next_property_set(Some(3)), Some(4)); // Like
        assert_eq!(data.next_property_set(Some(4)), None);
    }

    #[test]
    fn set_and_get_are_stubbed_with_a_clear_message() {
        let mut dss = Dss::new();
        dss.command("Set mode=snap");
        dss.command("Get mode");
        assert_eq!(dss.errors().len(), 2);
        assert!(
            dss.errors()
                .iter()
                .all(|e| e.contains("not implemented in the Phase 2 executive")),
            "{:?}",
            dss.errors()
        );
    }

    #[test]
    fn duplicate_new_edits_existing() {
        let mut dss = Dss::new();
        dss.command("New TCC_Curve.t npts=2 C_array=(1 2)");
        // A second "New" of the same name becomes an edit (DSS_OBJECT, no dups).
        dss.command("New TCC_Curve.t C_array=(7 8)");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(query(&mut dss, "tcc_curve.t.c_array"), "[ 7 8]");
    }

    #[test]
    fn clear_drops_objects() {
        let mut dss = Dss::new();
        dss.command("New TCC_Curve.t npts=2");
        dss.command("Clear");
        assert_eq!(query(&mut dss, "TCC_Curve.t.npts"), "Property Unknown");
    }

    #[test]
    fn unknown_parameter_is_reported() {
        let mut dss = Dss::new();
        dss.command("New TCC_Curve.t bogus=3");
        assert!(dss.errors().iter().any(|e| e.contains("Unknown parameter")));
    }

    #[test]
    fn parse_obj_name_splits_on_last_class_dot() {
        assert_eq!(
            parse_obj_name("TCC_Curve.test.npts"),
            ("TCC_Curve.test".to_string(), "npts".to_string())
        );
        assert_eq!(
            parse_obj_name("test.npts"),
            ("test".to_string(), "npts".to_string())
        );
        assert_eq!(parse_obj_name("npts"), (String::new(), "npts".to_string()));
    }
}

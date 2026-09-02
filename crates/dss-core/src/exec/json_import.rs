//! `Dss::circuit_from_json` — the whole-circuit AltDSS JSON *reader*, a port of
//! `Obj_Circuit_FromJSON_` and its helpers `loadClassFromJSON` / `busFromJSON`
//! (`.inputs/dss_capi/src/CAPI/CAPI_Obj.pas:2674-2983`), wrapped like the C
//! export `Circuit_FromJSON` (`CAPI_Circuit.pas:1114`). The inverse of
//! [`Dss::circuit_to_json`](super::Dss::circuit_to_json).
//!
//! Flow (Pascal `Obj_Circuit_FromJSON_`): `Clear`, apply `DefaultBaseFreq`,
//! `MakeNewCircuit(Name)`, run `PreCommands`, then for every class in
//! `DSSClassList` order load its object array (`loadClassFromJSON`), rebuild the
//! bus list, apply the optional `Bus` coordinate/base-kV array (`busFromJSON`),
//! and finally run `PostCommands`. Each object is created through the ordinary
//! `New` path and its properties applied by `ClassProps::fill_from_json` (the
//! `FillObjFromJSON` port) under the same foreign-class view + post-edit signal
//! tail every script edit uses.

use crate::report::export::json::{Json, parse_json};
use crate::report::save::dump::commands::PASCAL_CLASS_ORDER;

use super::command::apply_edit_signal_tail;
use super::*;

/// √3 — Pascal `SQRT3` (`busFromJSON` divides `kVLL` by it).
const SQRT3: f64 = 1.732_050_807_568_877_2;

impl Dss {
    /// Pascal `Circuit_FromJSON` / `Obj_Circuit_FromJSON_`: rebuild the whole
    /// circuit from an AltDSS JSON document. Returns `Err(msg)` for a malformed
    /// document or a wrong top-level shape (mirroring the Pascal `except on E`
    /// → `DoSimpleMsg 20230919`); per-object/property problems are recorded in
    /// [`Dss::errors`] and do not abort the whole load unless the Pascal path
    /// would (`DSS.ErrorNumber <> 0` after a class/pre/post step).
    ///
    /// `joptions` is accepted for signature parity with the Pascal but the only
    /// import-relevant bit (`DSSJSONOptions.Edit`) is applied internally by
    /// `loadClassFromJSON`; no public option changes the reader's behavior, so
    /// this wrapper takes none.
    pub fn circuit_from_json(&mut self, json: &str) -> Result<(), String> {
        let tree = match parse_json(json) {
            Ok(t) => t,
            Err(e) => {
                let msg = format!("Error converting data from JSON: {e}");
                self.errors.push(msg.clone());
                return Err(msg);
            }
        };
        let Json::Obj(root) = tree else {
            let msg =
                "Error converting data from JSON: Invalid JSON type, expected an object for the \
                 circuit."
                    .to_string();
            self.errors.push(msg.clone());
            return Err(msg);
        };

        // `DSS.DSSExecutive.Clear()`.
        self.command("clear");

        // `DefaultBaseFreq` (before `MakeNewCircuit`, so the new circuit adopts it).
        if let Some(v) = obj_find(&root, "DefaultBaseFreq") {
            self.default_base_freq = json_f64(v);
        }

        // `MakeNewCircuit(DSS, jckt.Get('Name', 'untitled'))`.
        let name = obj_find(&root, "Name")
            .and_then(json_str)
            .unwrap_or_else(|| "untitled".to_string());
        self.command(&format!("new circuit.{name}"));

        // `PreCommands` (array of strings, run in order; abort on error).
        if let Some(err) = self.run_command_array(&root, "PreCommands") {
            return Err(err);
        }
        if !self.errors.is_empty() {
            return Ok(());
        }

        // Each class in `DSSClassList` order (the export's `PASCAL_CLASS_ORDER`).
        for &class_name in PASCAL_CLASS_ORDER {
            let Some(&ci) = self.class_by_name.get(&class_name.to_ascii_lowercase()) else {
                continue;
            };
            let Some(arr) = obj_find(&root, class_name) else {
                continue;
            };
            let Json::Arr(items) = arr else {
                self.errors.push(format!(
                    "Error loading {class_name} record from JSON: expected an array."
                ));
                return Ok(());
            };
            let items = items.clone();
            self.load_class_from_json(ci, class_name, &items);
            if !self.errors.is_empty() {
                return Ok(());
            }
        }

        // "MakeBusList" — `if BusNameRedefined then ReprocessBusDefs`.
        self.do_make_bus_list_cmd();

        // Optional `Bus` coordinate / base-kV array.
        if let Some(bus_arr) = obj_find(&root, "Bus") {
            let Json::Arr(items) = bus_arr else {
                return Err("\"Bus\" must be an array of bus objects, if provided.".to_string());
            };
            for item in items.clone() {
                if let Json::Obj(members) = item {
                    // Pascal `busFromJSON` raises on the `kVLN`+`kVLL` conflict;
                    // with no `try/except` in `Obj_Circuit_FromJSON_` it propagates
                    // to the C wrapper `Circuit_FromJSON`, aborting the whole load
                    // (not just skipping the bus). Mirror that abort here.
                    self.bus_from_json(&members)?;
                } else {
                    return Err("\"Bus[]\" must be a bus object.".to_string());
                }
            }
        }

        // `PostCommands`.
        if let Some(err) = self.run_command_array(&root, "PostCommands") {
            return Err(err);
        }

        Ok(())
    }

    /// Run a top-level string array (`PreCommands`/`PostCommands`) through the
    /// executive, aborting on the first `ErrorNumber <> 0` (Pascal
    /// `DSS.DSSExecutive.ParseCommand`). Returns `Err` only on a wrong shape.
    fn run_command_array(&mut self, root: &[(String, Json)], key: &str) -> Option<String> {
        let v = obj_find(root, key)?;
        let Json::Arr(items) = v else {
            return Some(format!(
                "\"{key}\" must be an array of strings, if provided."
            ));
        };
        for item in items.clone() {
            let Some(cmd) = json_str(&item) else {
                return Some(format!("\"{key}\" must be an array of strings."));
            };
            self.command(&cmd);
            if !self.errors.is_empty() {
                break;
            }
        }
        None
    }

    /// Pascal `loadClassFromJSON`: create/edit every object of one class from its
    /// JSON array. The `specialFirst` rule (the VSource class) edits the
    /// auto-created `source` (element 0) as its first entry instead of adding a
    /// duplicate.
    fn load_class_from_json(&mut self, ci: usize, class_name: &str, arr: &[Json]) {
        let mut special_first = class_name.eq_ignore_ascii_case("Vsource");
        for (num, item) in arr.iter().enumerate() {
            let Json::Obj(members) = item else {
                self.errors.push(format!(
                    "JSON/{class_name}: unexpected format for object number {num}."
                ));
                return;
            };
            let name = obj_find(members, "Name")
                .or_else(|| obj_find(members, "name"))
                .and_then(json_str);
            let Some(name) = name else {
                self.errors.push(format!(
                    "JSON/{class_name}: missing \"Name\" from item {num}."
                ));
                return;
            };

            if special_first {
                special_first = false;
                // Edit the existing element 1 (`cls.ElementList.Get(1)`).
                self.active_class = Some(ci);
                self.classes[ci].active = Some(0);
            } else {
                // `obj_NewFromClass` — create with defaults but WITHOUT the
                // empty pre-fill edit (so an element whose recalc needs another
                // element, e.g. RegControl → its transformer, does not run its
                // `RecalcElementData` before `FillObjFromJSON` sets the ref). A
                // DSS_OBJECT duplicate becomes an edit of the existing object.
                if !self.create_object_no_edit(class_name, &name) {
                    // Creation failed (duplicate circuit element, reserved name);
                    // the error is already logged.
                    return;
                }
            }
            self.fill_active_from_json(ci, members);
            if !self.errors.is_empty() {
                return;
            }
        }
    }

    /// Apply a JSON object's properties to the currently-active object of class
    /// `ci` — the `FillObjFromJSON` call inside `loadSingleObj`, under the
    /// foreign-class view + `EndEdit` + post-edit signal tail every script edit
    /// runs. File-backed directives never occur here (the export renders arrays
    /// inline), so their handling is intentionally omitted.
    fn fill_active_from_json(&mut self, ci: usize, members: &[(String, Json)]) {
        let oi = match self.classes[ci].active {
            Some(oi) => oi,
            None => return,
        };
        let Dss {
            classes,
            circuit,
            aux_parser,
            vars,
            enums,
            errors,
            ..
        } = self;
        {
            // Split the registry: the active class is edited while every other
            // class is a read view for ObjectRef resolution (as in
            // `edit_active_inner`).
            let (left, rest) = classes.split_at_mut(ci);
            let (active_class, right) = rest.split_first_mut().expect("ci in range");
            let foreign = ForeignClasses {
                left,
                right,
                split: ci,
            };
            let DssClass {
                props,
                arena: objects,
                ..
            } = active_class;
            // Unlike the script edit path (`edit_active_inner`), Pascal
            // `FillObjFromJSON` does NOT call `BeginEdit` — only `EndEdit` — so it
            // never clears `DefaultAndUnedited` (`DSSClass.pas:1598`). A default
            // DSS_OBJECT (e.g. `spectrum.defaultload`) rebuilt from JSON therefore
            // stays flagged and is omitted from the re-export, matching the oracle
            // (its own round trip drops such objects too). We must NOT clear the
            // flag here.
            objects[oi].data_mut().begin_edit_boundary();
            {
                let mut eng = PropEngine {
                    parser: aux_parser,
                    vars,
                    enums,
                    errors,
                    foreign: Some(&foreign),
                    // JSON carries no outer-parser quote state; class hooks key
                    // off the value itself where the distinction matters
                    // ([`PropEngine::was_quoted`]).
                    was_quoted: false,
                };
                props.fill_from_json(&mut objects[oi], members, &mut eng);
            }
            // Pascal `RecalcElementData` (run by `EndEdit`) reads the live
            // `ActiveCircuit.Solution` globals; thread that snapshot in.
            let live_sys = circuit
                .as_ref()
                .map(crate::solution::solution::sys_ctx)
                .unwrap_or_else(crate::elements::traits::SysCtx::parse_default);
            objects[oi].end_edit(&live_sys);
        }
        // The split borrows are dead here; the shared post-edit tail (deferred
        // errors/abort, bus-name-redefined + Yprim signal propagation,
        // ref-actions) needs the full registry.
        apply_edit_signal_tail(classes, circuit, errors, ci, oi);
    }

    /// Pascal `busFromJSON` (`CAPI_Obj.pas:2865`): apply one `Bus` array entry's
    /// coordinates / base-kV / keep flag to the named bus. An unknown bus is
    /// silently skipped (`busIdx = 0 → Exit`). The `kVLN`+`kVLL` conflict raises,
    /// which upstream aborts the whole load — returned here as `Err`.
    fn bus_from_json(&mut self, members: &[(String, Json)]) -> Result<(), String> {
        let Some(ckt) = self.circuit.as_mut() else {
            return Ok(());
        };
        let Some(name) = obj_find(members, "Name").and_then(json_str) else {
            return Ok(());
        };
        let Some(idx) = ckt.bus_list.find(&name) else {
            return Ok(()); // Pascal leaves an unknown bus silent (`busIdx = 0 → Exit`).
        };
        let bus = &mut ckt.buses[idx];
        let mut kv_done = false;
        if let Some(v) = obj_find(members, "X") {
            bus.coord_defined = true;
            bus.x = json_f64(v);
        }
        if let Some(v) = obj_find(members, "Y") {
            bus.coord_defined = true;
            bus.y = json_f64(v);
        }
        if let Some(Json::Bool(b)) = obj_find(members, "Keep") {
            bus.keep = *b;
        }
        if let Some(v) = obj_find(members, "kVLN") {
            bus.kv_base = json_f64(v);
            kv_done = true;
        }
        if let Some(v) = obj_find(members, "kVLL") {
            if kv_done {
                return Err("Both \"kVLN\" and \"kVLL\" were specified.".to_string());
            }
            ckt.buses[idx].kv_base = json_f64(v) / SQRT3;
        }
        Ok(())
    }
}

/// Case-sensitive member lookup in a JSON object (fpjson `TJSONObject.Find`).
fn obj_find<'a>(members: &'a [(String, Json)], key: &str) -> Option<&'a Json> {
    members.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

fn json_f64(v: &Json) -> f64 {
    match v {
        Json::Float(f) => *f,
        Json::Int(i) => *i as f64,
        _ => 0.0,
    }
}

fn json_str(v: &Json) -> Option<String> {
    match v {
        Json::Str(s) => Some(s.clone()),
        _ => None,
    }
}

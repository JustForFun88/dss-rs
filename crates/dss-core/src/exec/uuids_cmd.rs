//! The `Uuids` command (Pascal `ExecHelper.pas` `DoUuidsCmd:4465`), the
//! `DefaultCircuitUUIDs` hook `DoExportCmd` runs on every export keyword
//! (`ExportCIMXML.pas:1276` / `ExportOptions.pas:188`), and `Export Uuids`
//! (`ExportResults.pas` `ExportUuids:2871`). PHASE8_PLAN WP8.6 step 6.

use super::*;
use crate::cim::{Uuid, UuidChoice, get_or_create_uuid};

impl Dss {
    /// Pascal `TExecHelper.DoUuidsCmd` (`ExecHelper.pas:4465-4535`): read a
    /// comma-CSV of `<fullname>, <uuid>` lines and preload each object's UUID.
    /// Resets the CIM hashed-key list first (`StartUuidList`,
    /// `ExecHelper.pas:4472` — even when the file turns out missing). A
    /// malformed UUID at an assignment site aborts the whole command with
    /// error 303 (see the loop below); a missing object / blank line is a
    /// silent no-op.
    pub(crate) fn do_uuids_cmd(&mut self) {
        {
            let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
            self.cim
                .start_uuid_list(ckt.buses.len() + 2 * ckt.num_devices);
        }
        self.parser.next_param(&self.vars);
        let param = self.parser.make_string(&self.vars);
        let path = {
            let p = Path::new(&param);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                self.current_dir.join(p)
            }
        };
        let Ok(content) = std::fs::read_to_string(&path) else {
            // Pascal error 242.
            self.errors
                .push(format!("UUIDs file: {param} does not exist"));
            return;
        };

        // AuxParser with a comma delimiter (`DSS.AuxParser.Delimiters := ','`),
        // restored by the Pascal `finally` — which runs even when a malformed
        // UUID aborts the command (below).
        self.aux_parser.set_delimiters(",");
        // FPC `StringToUuid` raises `EConvertError` only at the two ASSIGNMENT
        // sites — inside `AddHashedUuid` (`ExportCIMXML.pas:992`) and at
        // `pName.UUID := StringToUuid(UuidVal)` (`ExecHelper.pas:4529`, reached
        // only when the object was FOUND) — and the exception ABORTS the whole
        // command: remaining lines are not processed and `ProcessCommand`'s
        // except handler reports error 303 (`ExecCommands.pas:697-701`). A
        // missing object or a blank line never reaches a parse → silent no-op
        // (oracle-probed 2026-07-07).
        let mut convert_error: Option<String> = None;
        // FPC SysUtils `EConvertError` text for `StringToGUID` (the brace-wrap
        // above runs BEFORE the parse, so the braced form appears here).
        let econvert = |uuid_val: &str| format!("\"{uuid_val}\" is not a valid GUID value");
        for raw in content.lines() {
            let line = raw.trim_end_matches('\r');
            self.aux_parser.set_cmd_string(line);
            self.aux_parser.next_param(&self.vars);
            let name_val = self.aux_parser.make_string(&self.vars);
            self.aux_parser.next_param(&self.vars);
            let mut uuid_val = self.aux_parser.make_string(&self.vars);
            // Format the UUID properly: wrap a brace-less value in `{}`.
            if !uuid_val.contains('{') {
                uuid_val = format!("{{{uuid_val}}}");
            }
            if name_val.contains('=') {
                // A non-identified (CIM-only) object → the hashed-key list;
                // `AddHashedUuid` parses inside, so a malformed UUID aborts
                // here too (its `Err` carries the same `EConvertError` text).
                if let Err(e) = self.cim.add_hashed_uuid(&name_val, &uuid_val) {
                    convert_error = Some(e);
                    break;
                }
                continue;
            }
            // A descendant of TNamedObject: circuit / Bus.<name> / Class.<name>.
            // Resolve the object FIRST — Pascal parses the UUID only when
            // `pName <> NIL`, so an unknown object skips the parse entirely.
            let (dev_class, dev_name) =
                parse_object_class_and_name(&mut self.aux_parser, &self.vars, &name_val);
            if dev_class.eq_ignore_ascii_case("circuit") {
                if let Some(ckt) = self.circuit.as_mut() {
                    match Uuid::parse(&uuid_val) {
                        Some(u) => ckt.uuid = Some(u),
                        None => {
                            convert_error = Some(econvert(&uuid_val));
                            break;
                        }
                    }
                }
            } else if dev_class.eq_ignore_ascii_case("Bus") {
                let ckt = self.circuit.as_mut().expect("post-circuit dispatch");
                if let Some(idx) = ckt.bus_list.find(&dev_name) {
                    match Uuid::parse(&uuid_val) {
                        Some(u) => ckt.buses[idx].uuid = Some(u),
                        None => {
                            convert_error = Some(econvert(&uuid_val));
                            break;
                        }
                    }
                }
            } else if let Some(&ci) = self.class_by_name.get(&dev_class.to_ascii_lowercase()) {
                // Pascal sets `LastClassReferenced`/`ActiveDSSClass` as a side
                // effect of the lookup.
                self.active_class = Some(ci);
                if self.classes[ci].set_active(&dev_name) {
                    let oi = self.classes[ci].active.expect("just set active");
                    match Uuid::parse(&uuid_val) {
                        Some(u) => self.classes[ci].arena[oi].data_mut().set_uuid(u),
                        None => {
                            convert_error = Some(econvert(&uuid_val));
                            break;
                        }
                    }
                }
            }
            // Unknown class / unknown object: pName stays NIL — silently skipped.
        }
        // The Pascal local `finally` — runs on the abort path too.
        self.aux_parser.reset_delims();
        if let Some(emsg) = convert_error {
            // Pascal `ProcessCommand`'s except handler (`ExecCommands.pas:
            // 697-701`): `DoErrorMsg(..., 303)` over the full command string.
            // CRLF renders LF (the errors-240/267 convention, `exec/command.rs`);
            // `cmd_string()` carries the trailing space `SetCmdString` appends.
            self.errors.push(crate::diag::DssDiagnostic::msg(
                format!(
                    "Error 303 Reported From OpenDSS Intrinsic Function: \n\
                     ProcessCommand: Exception Raised While Processing DSS Command: \n\
                     {}\n\nError Description: \n{emsg}\n\nProbable Cause: \n\
                     Error in command string or circuit data.",
                    self.parser.cmd_string()
                ),
                Some(303),
            ));
        }
    }

    /// Pascal `TCIMExporter.DefaultCircuitUUIDs` (`ExportCIMXML.pas:1276`),
    /// which `DoExportCmd` calls on **every** export keyword
    /// (`ExportOptions.pas:188`): start the hashed list if not already
    /// started, read (lazily creating) the circuit UUID, and find-or-create
    /// the `Station=Station=1` / `GeoRgn=GeoRgn=1` / `SubGeoRgn=SubGeoRgn=1`
    /// keys. The returned ids only feed the CIM XML exporters (WPG.18).
    pub(crate) fn default_circuit_uuids(&mut self) {
        let ckt = self.circuit.as_mut().expect("post-circuit dispatch");
        if !self.cim.is_started() {
            self.cim
                .start_uuid_list(ckt.buses.len() + 2 * ckt.num_devices);
        }
        let _fdr_id = get_or_create_uuid(&mut ckt.uuid);
        let _sub_id = self.cim.get_dev_uuid(UuidChoice::Station, "Station", 1);
        let _rgn_id = self.cim.get_dev_uuid(UuidChoice::GeoRgn, "GeoRgn", 1);
        let _sub_geo_id = self.cim.get_dev_uuid(UuidChoice::SubGeoRgn, "SubGeoRgn", 1);
    }

    /// Pascal `ExportUuids` (`ExportResults.pas:2871-2960`): one `<FullName>
    /// {UUID}` row for the circuit, every bus, every ckt element, then the
    /// LineCode/WireData/LineGeometry/XfmrCode/LineSpacing/TSData/CNData
    /// catalogs, then the hashed keys (`WriteHashedUUIDs`). Reads lazily
    /// create any missing UUID (random v4 — which is why the golden fixture
    /// preloads every object via the `Uuids` command). The `finally` frees the
    /// hashed list; `GlobalResult` is NOT set (probe-proven: `Text.Result`
    /// stays empty after `export uuids`, unlike every other export).
    pub(crate) fn export_uuids_to_file(&mut self, explicit: &str) {
        let mut out = String::new();
        {
            let ckt = self.circuit.as_mut().expect("post-circuit dispatch");
            let id = get_or_create_uuid(&mut ckt.uuid);
            out.push_str(&format!("Circuit.{} {}\n", ckt.name, id.to_dss_string()));
            for bus in &mut ckt.buses {
                let id = get_or_create_uuid(&mut bus.uuid);
                out.push_str(&format!("Bus.{} {}\n", bus.name, id.to_dss_string()));
            }
        }
        let elems: Vec<ElemId> = self
            .circuit
            .as_ref()
            .expect("post-circuit dispatch")
            .ckt_elements
            .clone();
        for r in elems {
            let class_name = self.classes[r.class_ord()].props.class_name().to_string();
            let obj = self.classes[r.class_ord()].arena[r.index()].data_mut();
            let id = obj.uuid();
            out.push_str(&format!(
                "{}.{} {}\n",
                class_name,
                obj.name(),
                id.to_dss_string()
            ));
        }
        // The seven catalog classes, in the Pascal order.
        for cls in [
            "linecode",
            "wiredata",
            "linegeometry",
            "xfmrcode",
            "linespacing",
            "tsdata",
            "cndata",
        ] {
            let Some(&ci) = self.class_by_name.get(cls) else {
                continue;
            };
            let class_name = self.classes[ci].props.class_name().to_string();
            for oi in 0..self.classes[ci].arena.len() {
                let data = self.classes[ci].arena.obj_mut(oi).data_mut();
                let id = data.uuid();
                out.push_str(&format!(
                    "{}.{} {}\n",
                    class_name,
                    data.name(),
                    id.to_dss_string()
                ));
            }
        }
        self.cim.write_hashed_uuids(&mut out);
        self.write_export(explicit, "EXP_UUIDS.csv", &out);
        // Pascal `finally: DSS.CIMExporter.FreeUuidList` — unlike the CIM XML
        // export (whose free is commented out upstream), `Export Uuids` DOES
        // free the hashed list.
        self.cim.free_uuid_list();
    }
}

//! The `Reconductor` command (Pascal `TExecHelper.DoReconductorCmd`,
//! `ExecHelper.pas:4245`): re-apply a LineCode or Geometry (plus an optional
//! extra edit string) to every line on the meter-zone traceback path between
//! two lines. The path is the `ParentPDElement` chain the EnergyMeter zone
//! build records, so both lines must sit in the same meter zone.

use super::*;

impl Dss {
    /// Pascal `DoReconductorCmd`. Parameter table `ReconductorCommands =
    /// ['Line1', 'Line2', 'LineCode', 'Geometry', 'EditString', 'Nphases']`
    /// (`ExecHelper.pas:5063`); `Nphases = 0` (unset) applies the edit to every
    /// line on the path, a nonzero value filters by phase count.
    pub(super) fn do_reconductor_cmd(&mut self) {
        let commands = CommandList::new(
            [
                "Line1",
                "Line2",
                "LineCode",
                "Geometry",
                "EditString",
                "Nphases",
            ]
            .iter()
            .copied(),
        );
        let mut line1 = String::new();
        let mut line2 = String::new();
        let mut linecode = String::new();
        let mut geometry = String::new();
        let mut my_edit_string = String::new();
        let mut linecode_specified = false;
        let mut geometry_specified = false;
        let mut nphases = 0i32;

        let mut param_pointer = 0usize;
        let mut param_name = self.parser.next_param(&self.vars);
        let mut param = self.parser.make_string(&self.vars);
        while !param.is_empty() {
            if param_name.is_empty() {
                param_pointer += 1;
            } else {
                param_pointer = commands.get_command(&param_name).map_or(0, |i| i + 1);
            }
            match param_pointer {
                1 => line1 = param.clone(),
                2 => line2 = param.clone(),
                3 => {
                    linecode = param.clone();
                    linecode_specified = true;
                    geometry_specified = false;
                }
                4 => {
                    geometry = param.clone();
                    linecode_specified = false;
                    geometry_specified = true;
                }
                5 => my_edit_string = param.clone(),
                6 => nphases = self.parser.make_integer(&self.vars).unwrap_or(0),
                // Pascal error 28701.
                _ => self
                    .errors
                    .push(format!("Error: Unknown Parameter on command line: {param}")),
            }
            param_name = self.parser.next_param(&self.vars);
            param = self.parser.make_string(&self.vars);
        }

        // Pascal `StripClassName`: everything past the first period.
        let strip = |s: &str| match s.find('.') {
            Some(p) => s[p + 1..].to_string(),
            None => s.to_string(),
        };
        let line1 = strip(&line1);
        let line2 = strip(&line2);

        if line1.is_empty() || line2.is_empty() {
            // Pascal error 28702.
            self.errors
                .push("Both Line1 and Line2 must be specified!".to_string());
            return;
        }
        if !linecode_specified && !geometry_specified {
            // Pascal error 28703.
            self.errors
                .push("Either a new LineCode or a Geometry must be specified!".to_string());
            return;
        }

        let line_ci = *self
            .class_by_name
            .get("line")
            .expect("Line is a registered class");
        let find = |classes: &[DssClass], name: &str| -> Option<usize> {
            classes[line_ci]
                .name_to_idx
                .get(&name.to_lowercase())
                .copied()
        };
        let (Some(i1), Some(i2)) = (find(&self.classes, &line1), find(&self.classes, &line2))
        else {
            // Pascal error 28704 — one message, Line1 checked first.
            let missing = if find(&self.classes, &line1).is_none() {
                &line1
            } else {
                &line2
            };
            self.errors.push(format!("Line.{missing} not found."));
            return;
        };
        let r1 = ElemRef {
            cls: line_ci,
            idx: i1,
        };
        let r2 = ElemRef {
            cls: line_ci,
            idx: i2,
        };

        // Both lines must be in the same EnergyMeter zone (the zone build wrote
        // `meter_obj` + `parent_pd`).
        let meter = |s: &Self, r: ElemRef| -> Option<ElemRef> {
            s.classes[r.cls].objects[r.idx]
                .as_ckt_element()
                .and_then(|e| e.cd().meter_obj)
        };
        let (m1, m2) = (meter(self, r1), meter(self, r2));
        let (Some(m1), Some(m2)) = (m1, m2) else {
            // Pascal error 28705.
            self.errors.push(
                "Error: Both Lines must be in the same EnergyMeter zone. One or both are not in any meter zone."
                    .to_string(),
            );
            return;
        };
        if m1 != m2 {
            // Pascal error 28706 (`%s` = the meters' FullNames).
            let full = |s: &Self, r: ElemRef| {
                format!(
                    "{}.{}",
                    s.classes[r.cls].props.class_name(),
                    s.classes[r.cls].objects[r.idx].data().name()
                )
            };
            self.errors.push(format!(
                "Error: Line1 is in {} zone while Line2 is in {} zone. Both must be in the same Zone.",
                full(self, m1),
                full(self, m2)
            ));
            return;
        }

        // Pascal `isPathBetween`: walk the `ParentPDElement` chain. The two
        // ifs run in order, so when both directions hold (Line1 = Line2) the
        // second wins — reproduced.
        let path_between = |s: &Self, from: ElemRef, to: ElemRef| -> bool {
            let mut cur = Some(from);
            while let Some(r) = cur {
                if r == to {
                    return true;
                }
                cur = s.classes[r.cls].objects[r.idx]
                    .as_ckt_element()
                    .and_then(|e| e.cd().parent_pd);
            }
            false
        };
        let mut trace_direction = 0;
        if path_between(self, r1, r2) {
            trace_direction = 1;
        }
        if path_between(self, r2, r1) {
            trace_direction = 2;
        }

        let edit_string = if linecode_specified {
            format!("Linecode={linecode}")
        } else {
            format!("Geometry={geometry}")
        };
        // Pascal `Format('%s  %s', …)` — two spaces, MyEditString appended even
        // when empty.
        let edit_string = format!("{edit_string}  {my_edit_string}");

        match trace_direction {
            1 => self.trace_and_edit(r1, r2, nphases, &edit_string),
            2 => self.trace_and_edit(r2, r1, nphases, &edit_string),
            // Pascal error 28707.
            _ => self
                .errors
                .push("Traceback path not found between Line1 and Line2.".to_string()),
        }
    }

    /// Pascal `TraceAndEdit` (`Utilities.pas:1597`): walk up the
    /// `ParentPDElement` chain from `from` to `to` (inclusive), running the
    /// edit string against every element whose phase count matches (`nphases =
    /// 0` = no filter). The edit goes through the ordinary class-edit path
    /// (Pascal `pLine.Edit(DSS.Parser)`), so property side effects/PrpSequence
    /// update exactly like a user edit.
    fn trace_and_edit(&mut self, from: ElemRef, to: ElemRef, nphases: i32, edit_str: &str) {
        let mut cur = Some(from);
        while let Some(r) = cur {
            let elem_nphases = self.classes[r.cls].objects[r.idx]
                .as_ckt_element()
                .map(|e| e.cd().nphases as i32)
                .unwrap_or(0);
            if nphases == 0 || elem_nphases == nphases {
                self.parser.set_cmd_string(edit_str);
                self.active_class = Some(r.cls);
                self.classes[r.cls].active = Some(r.idx);
                self.edit_active();
            }
            if r == to {
                break;
            }
            cur = self.classes[r.cls].objects[r.idx]
                .as_ckt_element()
                .and_then(|e| e.cd().parent_pd);
        }
    }
}

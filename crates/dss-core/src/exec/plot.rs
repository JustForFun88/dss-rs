//! The Plot/Visualize callback surface — a port of `Executive/PlotOptions.pas`
//! (`DoPlotCmd`) plus the `AddBusMarker`/`ClearBusMarkers` commands
//! (`ExecHelper.pas`) that feed the plot payload's `BusMarkers[]`.
//!
//! The engine's job ends at assembling the `plotParams` JSON (the exact payload
//! Pascal builds with `TJSONObject.FormatJSON`) and handing it to the registered
//! plot callback (`DSS.DSSPlotCallback`). Actual rendering (matplotlib / plot
//! geometry) is out of scope by construction — the GUI consumer renders. With no
//! callback registered, `Plot` is a total no-op, byte-identical to the pinned
//! headless oracle (which runs with `DSSPlotCallback = NIL`).
//!
//! The fpjson serializer prints floats as `2.0000000000000000E+003` (16-digit
//! scientific); the Rust payload uses ordinary JSON numbers. That is a
//! formatting-only difference the golden comparator absorbs by parsing numbers
//! out (never a raw float-string diff) — not a `TODO(compat)` at this layer.

#[cfg(test)]
mod tests;

use serde_json::json;

use super::*;
use crate::circuit::BusMarker;
use crate::util::{color_to_html, interpret_color_name, interpret_yes_no};

/// The plot phase codes (`DSSClass.pas:326` `TPlotPhases`): the negative sentinels
/// selected by the `phases=` option. `ThreePhase = -1` is the default.
mod plot_phases {
    pub const LL_PRIMARY: i32 = -6;
    pub const LL_ALL: i32 = -5;
    pub const LL_3PH: i32 = -4;
    pub const PRIMARY: i32 = -3;
    pub const ALL: i32 = -2;
    pub const THREE_PHASE: i32 = -1;
}

/// Pascal `PlotOptions.TDSSPlot`: the parsed plot request. Initialized to the
/// `TDSSPlot.Create` defaults (PlotOptions.pas:104-150) and mutated by the parse
/// loop (:254-436); then serialized into the callback JSON (:472-533).
struct PlotParams {
    plot_type: String,
    matrix_type: String,
    max_scale: f64,
    min_scale: f64,
    dots: bool,
    labels: bool,
    show_loops: bool,
    show_subs: bool,
    quantity: String,
    object_name: String,
    plot_id: String,
    value_index: i32,
    phases_to_plot: i32,
    profile_scale: String,
    channels: Vec<u32>,
    bases: Vec<f64>,
    color1: i32,
    color2: i32,
    color3: i32,
    tri_color_max: f64,
    tri_color_mid: f64,
    max_scale_is_specified: bool,
    min_scale_is_specified: bool,
    daisy_bus_list: Vec<String>,
    max_line_thickness: i32,
    single_ph_line_style: i32,
    three_ph_line_style: i32,
}

impl Default for PlotParams {
    /// Pascal `TDSSPlot.Create` (PlotOptions.pas:104-150).
    fn default() -> Self {
        Self {
            plot_type: "Circuit".to_string(),
            matrix_type: String::new(),
            max_scale: 0.0,
            min_scale: 0.0,
            dots: false,
            labels: false,
            show_loops: false,
            show_subs: false,
            quantity: "Power".to_string(),
            object_name: String::new(),
            plot_id: String::new(),
            value_index: 0,
            phases_to_plot: plot_phases::THREE_PHASE,
            profile_scale: "pukm".to_string(),
            channels: vec![1, 3, 5],
            bases: vec![1.0, 1.0, 1.0],
            color1: crate::util::CL_BLUE_DEFAULT, // clBlue
            color2: 0x008000,                     // clGreen
            color3: 0x0000FF,                     // clRed
            tri_color_max: 0.85,
            tri_color_mid: 0.50,
            max_scale_is_specified: false,
            min_scale_is_specified: false,
            daisy_bus_list: Vec::new(),
            max_line_thickness: 10,
            single_ph_line_style: 1,
            three_ph_line_style: 1,
        }
    }
}

/// Pascal `InterpretTStringListArray` (the value-list branch, Utilities.pas:636):
/// tokenize `s` through a scratch parser into its whitespace/comma-separated
/// values. The `file=` branch (a file-backed daisy bus list) is not ported — no
/// corpus deck uses it and it is inert to the solve (a daisy-plot annotation
/// only); it would need filesystem access this hook does not have.
fn interpret_string_list(s: &str) -> Vec<String> {
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    parser.set_auto_increment(false);
    parser.set_cmd_string(s);
    let mut list = Vec::new();
    loop {
        parser.next_param(&vars);
        let token = parser.make_string(&vars);
        if token.is_empty() {
            break;
        }
        list.push(token);
    }
    list
}

impl Dss {
    /// Pascal `DoPlotCmd` (PlotOptions.pas:182). Parse the plot options and feed
    /// the assembled JSON to the registered callback.
    ///
    /// Pascal's two gates — `NoFormsAllowed` (:202) and `DSSPlotCallback = NIL`
    /// (:207), both exiting BEFORE any parsing — collapse to the single "is a
    /// callback registered?" check here (see [`Dss::register_plot_callback`]).
    /// The no-circuit #24731 guard is unreachable: the dispatcher's generic
    /// pre-circuit guard already emits #301 (oracle-probed), so `do_plot_cmd` is
    /// only reached with a circuit present.
    pub(super) fn do_plot_cmd(&mut self) {
        // Subsumes Pascal Gate A (NoFormsAllowed) + Gate B (callback NIL): with
        // no callback, Plot is a total no-op, exactly as before this port.
        if self.plot_callback.is_none() {
            return;
        }
        let Some(mut p) = self.parse_plot_options() else {
            return; // a solution guard tripped (#24732) — no callback fires
        };

        // Post-parse override (PlotOptions.pas:444): an unsolved circuit forces
        // `Quantity := 'None'`.
        let solved = self.circuit.as_ref().is_some_and(|c| c.is_solved);
        if !solved {
            p.quantity = "None".to_string();
        }

        let json = self.build_plot_json(&p);
        self.fire_plot_callback(&json);
    }

    /// The PlotOptions.pas:224-442 parse loop. Returns `None` when a solution
    /// guard (#24732) trips mid-parse (Pascal `Exit`, no callback).
    fn parse_plot_options(&mut self) -> Option<PlotParams> {
        let Dss {
            plot_commands,
            parser,
            vars,
            circuit,
            errors,
            ..
        } = self;
        let case_name = circuit
            .as_ref()
            .map(|c| c.case_name.clone())
            .unwrap_or_default();

        let mut p = PlotParams::default();
        let mut pointer: usize = 0;
        let mut param_name = parser.next_param(vars);
        let mut param_original = parser.make_string(vars);
        let mut param = param_original.to_uppercase();

        while !param.is_empty() {
            if param_name.is_empty() {
                pointer += 1;
            } else {
                pointer = plot_commands
                    .get_command(&param_name)
                    .map(|i| i + 1)
                    .unwrap_or(0);
            }

            // Solution guard (PlotOptions.pas:233-251): only for the `type=`
            // option, only when the first letter is A/C/D/G/M/P/Z and the value
            // is not a `pri…` prefix (PriceShape is allowed unsolved).
            if pointer == 1 {
                let first = param.as_bytes().first().copied().unwrap_or(0);
                let is_guarded = matches!(first, b'A' | b'C' | b'D' | b'G' | b'M' | b'P' | b'Z');
                if is_guarded && !crate::util::compare_text_shortest_eq("pri", &param) {
                    // #24731 (no circuit) is unreachable — the dispatcher emits
                    // #301 first — but the #24732 (unsolved) guard fires here.
                    match circuit.as_ref() {
                        None => {
                            errors.push("No circuit created.".to_string());
                            return None;
                        }
                        Some(c) if c.solution.node_v.len() <= 1 => {
                            errors.push(
                                "The circuit must be solved before you can do this.".to_string(),
                            );
                            return None;
                        }
                        Some(_) => {}
                    }
                }
            }

            match pointer {
                1 => parse_type(&mut p, &param, &case_name),
                2 => parse_quantity(&mut p, &param, parser, vars),
                3 => {
                    p.max_scale = dbl(parser, vars, errors);
                    p.max_scale_is_specified = p.max_scale > 0.0;
                }
                4 => p.dots = interpret_yes_no(&param),
                5 => p.labels = interpret_yes_no(&param),
                6 => p.object_name = param_original.clone(),
                7 => {
                    p.show_loops = interpret_yes_no(&param);
                    if p.show_loops {
                        p.plot_type = "MeterZones".to_string();
                    }
                }
                8 => p.tri_color_max = dbl(parser, vars, errors),
                9 => p.tri_color_mid = dbl(parser, vars, errors),
                10 => p.color1 = color(&param, errors),
                11 => p.color2 = color(&param, errors),
                12 => p.color3 = color(&param, errors),
                13 => {
                    // Channel definitions for a Plot Monitor (up to 50 channels).
                    let mut buf = [0.0f64; 51];
                    let n = parser.parse_as_vector(vars, &mut buf, false).unwrap_or(0);
                    if n > 0 {
                        let n = n.min(51);
                        p.channels = buf[..n]
                            .iter()
                            // FPC `Round` (banker's) into an `array of Cardinal`:
                            // a negative wraps modulo 2^32 (range checks off
                            // upstream), so go through i64 — `as u32` alone
                            // would saturate to 0 (audit settlement).
                            .map(|v| (v.round_ties_even() as i64) as u32)
                            .collect();
                        p.bases = vec![1.0; n];
                    }
                }
                14 => {
                    let mut buf = [0.0f64; 51];
                    let n = parser.parse_as_vector(vars, &mut buf, false).unwrap_or(0);
                    if n > 0 {
                        let n = n.min(51);
                        p.bases = buf[..n].to_vec();
                    }
                }
                15 => p.show_subs = interpret_yes_no(&param),
                16 => {
                    let v = int(parser, vars, errors);
                    if v > 0 {
                        p.max_line_thickness = v;
                    }
                }
                17 => p.daisy_bus_list = interpret_string_list(&param_original),
                18 => {
                    p.min_scale = dbl(parser, vars, errors);
                    // TODO(compat): PlotOptions.pas:397 always flags MinScale as
                    // specified, even for MinScale=0 (asymmetric with `max=`,
                    // which flags only when >0). Reproduced 1:1.
                    p.min_scale_is_specified = true;
                }
                19 => p.three_ph_line_style = int(parser, vars, errors),
                20 => p.single_ph_line_style = int(parser, vars, errors),
                21 => parse_phases(&mut p, &param, parser, vars, errors),
                22 => {
                    p.profile_scale = "pukm".to_string();
                    if crate::util::compare_text_shortest_eq(&param, "120KFT") {
                        p.profile_scale = "120kft".to_string();
                    }
                }
                23 => p.plot_id = param_original.clone(),
                _ => {}
            }

            param_name = parser.next_param(vars);
            param_original = parser.make_string(vars);
            param = param_original.to_uppercase();
        }
        Some(p)
    }

    /// Assemble the plot payload (PlotOptions.pas:447-536) as a JSON string. Key
    /// order is not significant (the golden comparator is structural).
    fn build_plot_json(&self, p: &PlotParams) -> String {
        let ckt = self.circuit.as_ref();
        let bus_markers: Vec<_> = ckt
            .map(|c| c.bus_marker_list.as_slice())
            .unwrap_or(&[])
            .iter()
            .map(|m| {
                json!({
                    "Name": m.bus_name,
                    "Color": color_to_html(m.add_marker_color),
                    "Code": m.add_marker_code,
                    "Size": m.add_marker_size,
                })
            })
            .collect();

        // The `Markers` object (PlotOptions.pas:501-531): the circuit's GUI
        // marker-style globals (Circuit.pas defaults for every headless run).
        let markers = ckt.map(|c| {
            json!({
                "NodeMarkerCode": c.node_marker_code,
                "NodeMarkerWidth": c.node_marker_width,
                "SwitchMarkerCode": c.switch_marker_code,
                "TransMarkerSize": c.trans_marker_size,
                "CapMarkerSize": c.cap_marker_size,
                "RegMarkerSize": c.reg_marker_size,
                "PVMarkerSize": c.pv_marker_size,
                "StoreMarkerSize": c.store_marker_size,
                "FuseMarkerSize": c.fuse_marker_size,
                "RecloserMarkerSize": c.recloser_marker_size,
                "RelayMarkerSize": c.relay_marker_size,
                "TransMarkerCode": c.trans_marker_code,
                "CapMarkerCode": c.cap_marker_code,
                "RegMarkerCode": c.reg_marker_code,
                "PVMarkerCode": c.pv_marker_code,
                "StoreMarkerCode": c.store_marker_code,
                "FuseMarkerCode": c.fuse_marker_code,
                "RecloserMarkerCode": c.recloser_marker_code,
                "RelayMarkerCode": c.relay_marker_code,
                "MarkSwitches": c.mark_switches,
                "MarkTransformers": c.mark_transformers,
                "MarkCapacitors": c.mark_capacitors,
                "MarkRegulators": c.mark_regulators,
                "MarkPVSystems": c.mark_pv_systems,
                "MarkStorage": c.mark_storage,
                "MarkFuses": c.mark_fuses,
                "MarkReclosers": c.mark_reclosers,
                "MarkRelays": c.mark_relays,
            })
        });

        let payload = json!({
            "PlotType": p.plot_type,
            "MatrixType": p.matrix_type,
            "MaxScale": p.max_scale,
            "MinScale": p.min_scale,
            "Dots": p.dots,
            "Labels": p.labels,
            "ShowLoops": p.show_loops,
            "ShowSubs": p.show_subs,
            "Quantity": p.quantity,
            "ObjectName": p.object_name,
            "PlotId": p.plot_id,
            "ValueIndex": p.value_index,
            "PhasesToPlot": p.phases_to_plot,
            "ProfileScale": p.profile_scale,
            "Channels": p.channels,
            "Bases": p.bases,
            "SinglePhLineStyle": p.single_ph_line_style,
            "ThreePhLineStyle": p.three_ph_line_style,
            "Color1": color_to_html(p.color1),
            "Color2": color_to_html(p.color2),
            "Color3": color_to_html(p.color3),
            "TriColorMax": p.tri_color_max,
            "TriColorMid": p.tri_color_mid,
            "MaxScaleIsSpecified": p.max_scale_is_specified,
            "MinScaleIsSpecified": p.min_scale_is_specified,
            "DaisyBusList": p.daisy_bus_list,
            "DaisySize": self.daisy_size,
            "MaxLineThickness": p.max_line_thickness,
            "Markers": markers,
            "BusMarkers": bus_markers,
        });
        payload.to_string()
    }

    /// Invoke the registered plot callback with the assembled JSON (Pascal
    /// `DSS.DSSPlotCallback(DSS, PChar(plotParamsStr))`; the returned `Integer`
    /// is ignored, as upstream).
    pub(super) fn fire_plot_callback(&mut self, json: &str) {
        if let Some(cb) = self.plot_callback.as_mut() {
            let _ = cb(json);
        }
    }

    /// Build the `Visualize` payload (`ExecHelper.pas:4184-4189`) for a resolved
    /// circuit element and fire the callback. Called from `do_visualize_cmd`
    /// after the found/guard logic, only when the element exists.
    pub(super) fn fire_visualize_callback(
        &mut self,
        element_name: &str,
        element_type: &str,
        quantity: &str,
    ) {
        if self.plot_callback.is_none() {
            return;
        }
        let json = json!({
            "PlotType": "Visualize",
            "ElementName": element_name,
            "ElementType": element_type,
            "Quantity": quantity,
        })
        .to_string();
        self.fire_plot_callback(&json);
    }

    /// Pascal `DoAddMarkerCmd` (`ExecHelper.pas:4382`): create a `TBusMarker`,
    /// append it to the circuit's `BusMarkerList`, and parse `Bus/code/color/size`
    /// (`AddMarkerCommands = ['Bus','code','color','size']`, abbreviation-matched).
    pub(super) fn do_add_marker_cmd(&mut self) {
        // Pascal appends the marker before parsing (an empty command still adds a
        // default marker). Guard on a circuit existing (dispatched post-circuit).
        let Dss {
            parser,
            vars,
            circuit,
            errors,
            ..
        } = self;
        let Some(ckt) = circuit.as_mut() else {
            return;
        };
        ckt.bus_marker_list.push(BusMarker::default());
        let marker_idx = ckt.bus_marker_list.len() - 1;

        // `AddMarkerCommands = ['Bus','code','color','size']`. Small local list
        // so ownership matches the oracle (abbreviation-matched like Pascal).
        let cmds = CommandList::new(["Bus", "code", "color", "size"]);
        let mut pointer: usize = 0;
        let mut param_name = parser.next_param(vars);
        let mut param = parser.make_string(vars);
        while !param.is_empty() {
            if param_name.is_empty() {
                pointer += 1;
            } else {
                pointer = cmds.get_command(&param_name).map(|i| i + 1).unwrap_or(0);
            }
            let m = &mut ckt.bus_marker_list[marker_idx];
            match pointer {
                1 => m.bus_name = param.clone(),
                2 => m.add_marker_code = parser.make_integer(vars).unwrap_or(0),
                3 => m.add_marker_color = color(&param, errors),
                4 => m.add_marker_size = parser.make_integer(vars).unwrap_or(0),
                _ => {}
            }
            param_name = parser.next_param(vars);
            param = parser.make_string(vars);
        }
    }

    /// Pascal `TDSSCircuit.ClearBusMarkers` (`Circuit.pas:3069`): empty the
    /// bus-marker list.
    pub(super) fn do_clear_bus_markers_cmd(&mut self) {
        if let Some(ckt) = self.circuit.as_mut() {
            ckt.bus_marker_list.clear();
        }
    }
}

/// Parse the `type=` option (PlotOptions.pas:256-306) by first letter.
fn parse_type(p: &mut PlotParams, param: &str, case_name: &str) {
    let cts = |sub: &str| crate::util::compare_text_shortest_eq(sub, param);
    match param.as_bytes().first().copied().unwrap_or(0) {
        b'A' => {
            p.plot_type = "AutoAddLog".to_string();
            p.object_name = format!("{case_name}_AutoAddLog.csv"); // DSS.CircuitName_ + …
            p.value_index = 2;
        }
        b'C' => p.plot_type = "Circuit".to_string(),
        b'E' => {
            p.plot_type = if cts("ener") { "Energy" } else { "Evolution" }.to_string();
        }
        b'G' => p.plot_type = "GeneralData".to_string(),
        // TODO(compat): PlotOptions.pas:274 maps every `L…` unconditionally to
        // LoadShape — so `type=Losses` is a LoadShape plot, not a losses plot.
        // Reproduced 1:1 (oracle-confirmed).
        b'L' => p.plot_type = "LoadShape".to_string(),
        b'M' => {
            p.plot_type = if cts("mon") { "Monitor" } else { "Matrix" }.to_string();
        }
        b'P' => {
            p.plot_type = if cts("pro") {
                "Profile"
            } else if cts("phas") {
                "PhaseVoltage"
            } else {
                "PriceShape"
            }
            .to_string();
        }
        b'S' => p.plot_type = "Scatter".to_string(),
        b'T' => p.plot_type = "TShape".to_string(),
        b'D' => {
            p.plot_type = "Daisy".to_string();
            p.daisy_bus_list.clear();
        }
        b'Z' => p.plot_type = "MeterZones".to_string(),
        // TODO(compat): PlotOptions.pas:305 has an empty `else` — an unrecognized
        // first letter leaves PlotType at its default ('Circuit'). Reproduced.
        _ => {}
    }
}

/// Parse the `quantity=` option (PlotOptions.pas:307-332).
fn parse_quantity(p: &mut PlotParams, param: &str, parser: &mut Parser, vars: &ParserVars) {
    let bytes = param.as_bytes();
    match bytes.first().copied().unwrap_or(0) {
        b'V' => p.quantity = "Voltages".to_string(),
        b'C' => match bytes.get(1).copied().unwrap_or(0) {
            b'A' => p.quantity = "Capacities".to_string(),
            b'U' => p.quantity = "Currents".to_string(),
            _ => {}
        },
        b'P' => p.quantity = "Powers".to_string(),
        b'L' => {
            if crate::util::compare_text_shortest_eq("los", param) {
                p.quantity = "Losses".to_string();
            } else {
                p.matrix_type = "Laplacian".to_string();
            }
        }
        b'I' => p.matrix_type = "IncMatrix".to_string(),
        _ => {
            p.quantity = "None".to_string();
            p.value_index = parser.make_integer(vars).unwrap_or(0);
        }
    }
}

/// Parse the `phases=` option (PlotOptions.pas:403-426).
fn parse_phases(
    p: &mut PlotParams,
    param: &str,
    parser: &mut Parser,
    vars: &ParserVars,
    _errors: &mut [String],
) {
    let cts = |name: &str| crate::util::compare_text_shortest_eq(param, name);
    p.phases_to_plot = plot_phases::THREE_PHASE; // the default
    if cts("default") {
        p.phases_to_plot = plot_phases::THREE_PHASE;
    } else if cts("all") {
        p.phases_to_plot = plot_phases::ALL;
    } else if cts("primary") {
        p.phases_to_plot = plot_phases::PRIMARY;
    } else if cts("ll3ph") {
        p.phases_to_plot = plot_phases::LL_3PH;
    } else if cts("llall") {
        p.phases_to_plot = plot_phases::LL_ALL;
    } else if cts("llprimary") {
        p.phases_to_plot = plot_phases::LL_PRIMARY;
    } else if param.len() == 1 {
        p.phases_to_plot = parser.make_integer(vars).unwrap_or(0);
    }
}

/// `Parser.DblValue` of the current token, recording any conversion error.
fn dbl(parser: &mut Parser, vars: &ParserVars, errors: &mut Vec<String>) -> f64 {
    match parser.make_double(vars) {
        Ok(v) => v,
        Err(e) => {
            errors.push(e.message().to_string());
            0.0
        }
    }
}

/// `Parser.IntValue` of the current token, recording any conversion error.
fn int(parser: &mut Parser, vars: &ParserVars, errors: &mut Vec<String>) -> i32 {
    match parser.make_integer(vars) {
        Ok(v) => v,
        Err(e) => {
            errors.push(e.message().to_string());
            0
        }
    }
}

/// `InterpretColorName` with the Pascal error/fallback: an invalid spec pushes
/// error #724 and returns `clBlue`.
fn color(param: &str, errors: &mut Vec<String>) -> i32 {
    match interpret_color_name(param) {
        Some(c) => c,
        None => {
            errors.push(format!("Invalid Color Specification: \"{param}\"."));
            crate::util::CL_BLUE_DEFAULT
        }
    }
}

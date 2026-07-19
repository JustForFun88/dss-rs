//! The generic scripting surface of `epri-worker` — the `exec` / `read` /
//! `chdir` protocol commands used by the manual regen drivers and probes
//! (`tools/opendss/epri_worker.py`: `tools/golden/gen_protection.py` r4133 arm,
//! `tools/golden/gen_flicker.py`, `tools/opendss/probe_59n.py`).
//!
//! This is the functional-parity replacement for the retired Oddie/dss-python
//! bridge's ad-hoc scripting (`d.Text.Command` + individual API property reads):
//! each `read` maps to exactly one engine accessor and polls the error state per
//! call, mirroring dss-python's raise-on-nonzero after every property access.
//! **Gate-neutral by construction**: the `run`/`ping`/`quit`/`clear` handlers and
//! [`crate::capture`] are untouched; these commands are never sent by
//! `corpus_gate` schedulers.
//!
//! The EPRI capability round (Round 2) adds two generic handlers here:
//! [`handle_ffi`] (any DDLL family mode via `(family, kind, mode, arg)` — the
//! capability-complete channel of [`crate::families`]) and [`handle_ymatrix`]
//! (the standalone `YMatrix.*` helpers). Both are additive and gate-neutral.

use serde_json::{Value, json};

use crate::dss::{Engine, EngineError, FfiCall, FfiOut};
use crate::families::VData;

/// Dispatch one `{"cmd":"read","what":…}` request against the engine. Every
/// branch finishes with an error poll ([`Engine::assert_clean`]) so a failed
/// read escalates instead of returning garbage (dss-python per-call parity).
pub fn handle_read(engine: &Engine, req: &Value) -> Result<Value, EngineError> {
    let what = req
        .get("what")
        .and_then(|v| v.as_str())
        .ok_or_else(|| EngineError::Other("read: missing `what`".to_string()))?;
    let name = req.get("name").and_then(|v| v.as_str());
    let need_name = |what: &str| {
        name.map(str::to_string)
            .ok_or_else(|| EngineError::Other(format!("read {what}: missing `name`")))
    };
    let v = match what {
        // -- solution scalars (Oddie `sol.dblHour` / `.Iterations` / `.Converged`)
        "dbl_hour" => json!(engine.dbl_hour()),
        "iterations" => json!(engine.iterations()),
        "converged" => json!(engine.converged()),
        // -- circuit-level arrays
        "ynode_order" => json!(engine.ynode_order()),
        "ynode_varray" => json!(engine.ynode_varray()),
        "all_element_names" => json!(engine.all_element_names()),
        // -- active-element cursor + reads (Oddie `ckt.SetActiveElement` +
        //    `ActiveCktElement.Powers` / `.Currents` — same one-call-per-read
        //    sequence, so golden captures replay the identical DLL call order)
        "set_active_element" => {
            engine.set_active_element(&need_name(what)?);
            json!(true)
        }
        "element_powers" => json!(engine.element_powers()),
        "element_currents" => json!(engine.element_currents()),
        // -- element state variables (Oddie `AllVariableNames`/`AllVariableValues`)
        "variables" => {
            engine.set_active_element(&need_name(what)?);
            json!({
                "var_names": engine.element_variable_names(),
                "values": engine.element_variable_values(),
            })
        }
        // -- event log (Oddie `sol.EventLog` — see `Engine::eventlog` decode note)
        "eventlog" => json!(engine.eventlog()),
        // -- monitor-by-name reads (Oddie `mon.Name = …` + `SampleCount` /
        //    `NumChannels` / `Channel(k)`)
        "monitor_select" => {
            engine.monitor_select(&need_name(what)?);
            json!(true)
        }
        "monitor_sample_count" => json!(engine.monitor_sample_count()),
        "monitor_num_channels" => json!(engine.monitor_num_channels()),
        "monitor_channel" => {
            let idx = req.get("index").and_then(|v| v.as_i64()).ok_or_else(|| {
                EngineError::Other("read monitor_channel: missing `index`".into())
            })?;
            json!(engine.monitor_channel(idx as i32))
        }
        // -- bus kVBase (Oddie `ckt.SetActiveBus` + `ActiveBus.kVBase`)
        "bus_kvbase" => {
            let bus = need_name(what)?;
            if engine.set_active_bus(&bus) < 0 {
                return Err(EngineError::Other(format!("bus {bus:?} not found")));
            }
            json!(engine.bus_kvbase())
        }
        other => {
            return Err(EngineError::Other(format!("unknown read what {other:?}")));
        }
    };
    engine.assert_clean(&format!("read {what}"))?;
    Ok(v)
}

/// Dispatch one `{"cmd":"ffi","family":…,"kind":"i"|"f"|"s"|"v","mode":…}` request
/// — the generic capability channel that reaches every mode of every DDLL family
/// (see [`crate::families`]). Scalar arg by kind: `iarg` (i) / `farg` (f) /
/// `sarg` (s). For a `"v"` call, an optional `{"vset":{"type":…,"data":[…]}}`
/// drives the array-SET mode; absent = the getter. The reply always carries the
/// **structured errno surface** (`errno` + `error`) polled right after the call —
/// something the Python bridge never exposed per-call.
pub fn handle_ffi(engine: &Engine, req: &Value) -> Result<Value, EngineError> {
    let family = req
        .get("family")
        .and_then(|v| v.as_str())
        .ok_or_else(|| EngineError::Other("ffi: missing `family`".to_string()))?;
    let kind = req
        .get("kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| EngineError::Other("ffi: missing `kind`".to_string()))?
        .to_ascii_lowercase();
    let mode = req
        .get("mode")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| EngineError::Other("ffi: missing/!int `mode`".to_string()))?
        as i32;
    let iarg = req.get("iarg").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let farg = req.get("farg").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let sarg = req.get("sarg").and_then(|v| v.as_str()).unwrap_or("");
    let vset = parse_vset(req.get("vset"))?;

    let out = engine.ffi_dispatch(FfiCall {
        family,
        kind: &kind,
        mode,
        iarg,
        farg,
        sarg,
        vset,
    })?;
    // Structured errno surface (beyond the Python bridge): poll right after.
    let (errno, desc) = engine.poll_error();
    let mut result = ffi_out_to_json(&out);
    result["errno"] = json!(errno);
    result["error"] = json!(desc);
    Ok(result)
}

/// Dispatch one `{"cmd":"ymatrix","op":…}` request — the standalone Y-matrix /
/// injection helpers (`DYMatrix.pas`) that fall outside the uniform family shape
/// (dss-python's `YMatrix.*`). Read-oriented ops return values; the injection /
/// build ops return `{"ok":true}`.
pub fn handle_ymatrix(engine: &Engine, req: &Value) -> Result<Value, EngineError> {
    let op = req
        .get("op")
        .and_then(|v| v.as_str())
        .ok_or_else(|| EngineError::Other("ymatrix: missing `op`".to_string()))?;
    // Several DYMatrix ops deref `ActiveCircuit[ActiveActor].Solution` with no
    // nil check in the DLL (`SystemYChanged`/`UseAuxCurrents`/`AddInAuxCurrents`/
    // `BuildYMatrixD`/`getVpointer`/`getIpointer`/`SolveSystem`), so invoking
    // them before a circuit is compiled would nil-deref and crash the worker.
    // Guard the crash set here (the Rust engine is safe; the DLL is not).
    let needs_circuit = matches!(
        op,
        "system_y_changed"
            | "use_aux_currents"
            | "build_y"
            | "add_aux"
            | "vpointer"
            | "ipointer"
            | "solve_system"
    );
    if needs_circuit && engine.circuit_name().is_empty() {
        return Err(EngineError::Other(format!(
            "ymatrix {op:?}: no circuit compiled"
        )));
    }
    let iarg = |k: &str| req.get(k).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let v = match op {
        "system_y_changed" => json!(engine.ym_system_y_changed(iarg("mode"), iarg("arg"))),
        "use_aux_currents" => json!(engine.ym_use_aux_currents(iarg("mode"), iarg("arg"))),
        "build_y" => {
            engine.ym_build_y(iarg("build_ops"), iarg("allocate_vi"));
            json!({"built": true})
        }
        "zero_inj" => {
            engine.ym_zero_inj();
            json!({"done": true})
        }
        "get_source_inj" => {
            engine.ym_get_source_inj();
            json!({"done": true})
        }
        "get_pc_inj" => {
            engine.ym_get_pc_inj();
            json!({"done": true})
        }
        "add_aux" => {
            engine.ym_add_aux(iarg("stype"));
            json!({"done": true})
        }
        "y_dims" => match engine.y_dims() {
            Some((n_bus, n_nz)) => json!({"n_bus": n_bus, "n_nz": n_nz}),
            None => json!(null),
        },
        "vpointer" => json!(engine.v_pointer(engine.num_nodes())),
        "ipointer" => json!(engine.injection_raw(engine.num_nodes())),
        "solve_system" => json!({"status": engine.solve_system(engine.num_nodes())}),
        other => return Err(EngineError::Other(format!("unknown ymatrix op {other:?}"))),
    };
    let (errno, desc) = engine.poll_error();
    Ok(json!({"result": v, "errno": errno, "error": desc}))
}

/// Serialize an [`FfiOut`] into the reply body (kind-tagged; `type`/`n` carried
/// for V arrays so the caller can round-trip the exact `myType`).
fn ffi_out_to_json(out: &FfiOut) -> Value {
    match out {
        FfiOut::I(v) => json!({"kind": "i", "value": v}),
        FfiOut::F(v) => json!({"kind": "f", "value": v}),
        FfiOut::S(v) => json!({"kind": "s", "value": v}),
        FfiOut::V(d) => {
            let mut o = json!({"kind": "v", "type": d.type_tag(), "n": d.len()});
            o["data"] = vdata_to_json(d);
            o
        }
        FfiOut::VSet(n) => json!({"kind": "v", "set": true, "written": n}),
    }
}

/// The `data` payload for a decoded V array.
fn vdata_to_json(d: &VData) -> Value {
    match d {
        VData::Ints(v) => json!(v),
        VData::Doubles(v) | VData::Complex(v) => json!(v),
        VData::Strings(v) => json!(v),
        VData::Bytes(v) => json!(v),
    }
}

/// Parse an optional `{"type":<1|2|3|4|5>,"data":[…]}` array-SET input.
fn parse_vset(v: Option<&Value>) -> Result<Option<VData>, EngineError> {
    let Some(obj) = v else { return Ok(None) };
    if obj.is_null() {
        return Ok(None);
    }
    let ty = obj
        .get("type")
        .and_then(|t| t.as_i64())
        .ok_or_else(|| EngineError::Other("ffi vset: missing/!int `type`".to_string()))?;
    let data = obj
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| EngineError::Other("ffi vset: missing/!array `data`".to_string()))?;
    let bad = || EngineError::Other("ffi vset: `data` element wrong type for `type`".to_string());
    let out = match ty {
        1 => VData::Ints(
            data.iter()
                .map(|x| x.as_i64().map(|n| n as i32).ok_or_else(bad))
                .collect::<Result<_, _>>()?,
        ),
        2 => VData::Doubles(
            data.iter()
                .map(|x| x.as_f64().ok_or_else(bad))
                .collect::<Result<_, _>>()?,
        ),
        3 => VData::Complex(
            data.iter()
                .map(|x| x.as_f64().ok_or_else(bad))
                .collect::<Result<_, _>>()?,
        ),
        4 => VData::Strings(
            data.iter()
                .map(|x| x.as_str().map(str::to_string).ok_or_else(bad))
                .collect::<Result<_, _>>()?,
        ),
        5 => VData::Bytes(
            data.iter()
                .map(|x| x.as_i64().map(|n| n as u8).ok_or_else(bad))
                .collect::<Result<_, _>>()?,
        ),
        other => {
            return Err(EngineError::Other(format!(
                "ffi vset: unknown type {other}"
            )));
        }
    };
    Ok(Some(out))
}

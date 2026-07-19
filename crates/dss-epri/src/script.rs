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

use serde_json::{Value, json};

use crate::dss::{Engine, EngineError};

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

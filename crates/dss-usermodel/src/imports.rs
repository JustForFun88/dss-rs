//! The `dss_env` host-import module: the Pascal 32-slot `TDSSCallBacks`
//! vtable (`Common/DSSCallBackRoutines.pas:19-67`) as wasm host functions,
//! one per slot, tiered per `docs/wasm/USERMODEL_ABI.md` §4.
//!
//! Every import exists at link time so modules validate; the
//! unimplemented-by-design slots (`do_dss_command`/`get_result_str` until
//! WP-WM.6, `get_active_element_ptr` permanently) raise the loud attributed
//! [`Fault::Unsupported`] when *called* — never a silent no-op (plan §2.9-5).
//!
//! Marshalling template: the vendored typst plugin host
//! (`.inputs/typst/crates/typst-library/src/foundations/plugin.rs:576-612`) —
//! host functions fetch the guest's exported `memory` from the `Caller`,
//! read/write linear memory directly, and record a typed fault in the store
//! data before trapping on OOB (typst's `memory_error` pattern).

use num_complex::Complex64;
use wasmi::{Caller, Extern, Linker, Memory};

use crate::callbacks::{CallData, Effect, Fault};

/// Fetch the guest's exported linear memory (typst `plugin.rs:581/:603`;
/// its presence is validated at module load, ABI doc §1).
fn memory(caller: &mut Caller<'_, CallData>) -> Result<Memory, wasmi::Error> {
    caller
        .get_export("memory")
        .and_then(Extern::into_memory)
        .ok_or_else(|| wasmi::Error::new("guest does not export `memory`"))
}

/// Record a typed OOB fault and produce the trap error (ABI doc §6:
/// protocol violation, hard and loud).
fn oob(caller: &mut Caller<'_, CallData>, import: &'static str, detail: String) -> wasmi::Error {
    caller.data_mut().fault = Some(Fault::OutOfBounds {
        import,
        detail: detail.clone(),
    });
    wasmi::Error::new(format!("dss_env.{import}: {detail}"))
}

/// Record the unsupported-import fault and produce the trap error
/// (ABI doc §4 rows 7/30/32).
fn unsupported(caller: &mut Caller<'_, CallData>, import: &'static str) -> wasmi::Error {
    caller.data_mut().fault = Some(Fault::Unsupported { import });
    wasmi::Error::new(format!("dss_env.{import}: not supported over WASM"))
}

/// Write `bytes` at guest pointer `ptr`.
fn write_guest(
    caller: &mut Caller<'_, CallData>,
    import: &'static str,
    ptr: i32,
    bytes: &[u8],
) -> Result<(), wasmi::Error> {
    let mem = memory(caller)?;
    mem.write(&mut *caller, ptr as u32 as usize, bytes)
        .map_err(|_| {
            oob(
                caller,
                import,
                format!(
                    "write of {} bytes at guest pointer {:#x} is out of bounds",
                    bytes.len(),
                    ptr as u32
                ),
            )
        })
}

/// Read `len` bytes at guest pointer `ptr`.
fn read_guest(
    caller: &mut Caller<'_, CallData>,
    import: &'static str,
    ptr: i32,
    len: usize,
) -> Result<Vec<u8>, wasmi::Error> {
    let mem = memory(caller)?;
    let mut buf = vec![0u8; len];
    mem.read(&*caller, ptr as u32 as usize, &mut buf)
        .map_err(|_| {
            oob(
                caller,
                import,
                format!(
                    "read of {len} bytes at guest pointer {:#x} is out of bounds",
                    ptr as u32
                ),
            )
        })?;
    Ok(buf)
}

/// Pascal `StrLCopy` semantics (ABI doc "Conventions"): copy at most `maxlen`
/// bytes of `s` to `ptr`, then a terminating NUL (the caller allocates
/// `maxlen + 1`, like the native contract). Returns the copied length.
fn str_lcopy(
    caller: &mut Caller<'_, CallData>,
    import: &'static str,
    ptr: i32,
    maxlen: i32,
    s: &str,
) -> Result<i32, wasmi::Error> {
    let maxlen = maxlen.max(0) as usize;
    let bytes = s.as_bytes();
    let n = bytes.len().min(maxlen);
    let mut out = Vec::with_capacity(n + 1);
    out.extend_from_slice(&bytes[..n]);
    out.push(0);
    write_guest(caller, import, ptr, &out)?;
    Ok(n as i32)
}

/// Serialize a complex as the 16-byte LE image (ABI doc "Conventions").
fn cplx_bytes(c: Complex64) -> [u8; 16] {
    let mut b = [0u8; 16];
    b[..8].copy_from_slice(&c.re.to_le_bytes());
    b[8..].copy_from_slice(&c.im.to_le_bytes());
    b
}

/// Serialize a complex slice (element k at `(k-1)*16`, the 1-based Pascal
/// array semantics of the ABI doc).
fn cplx_slice_bytes(cs: &[Complex64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(cs.len() * 16);
    for c in cs {
        out.extend_from_slice(&cplx_bytes(*c));
    }
    out
}

/// Read a guest i32 (LE) at `ptr`.
fn read_i32(
    caller: &mut Caller<'_, CallData>,
    import: &'static str,
    ptr: i32,
) -> Result<i32, wasmi::Error> {
    let b = read_guest(caller, import, ptr, 4)?;
    Ok(i32::from_le_bytes(b.try_into().expect("4-byte read")))
}

/// Bytes of the guest string at `(ptr, len)`, decoded as ANSI/latin-1-ish
/// lossy UTF-8 (upstream strings are ASCII by convention,
/// `GenUserModel.pas:9-11`).
fn read_str(
    caller: &mut Caller<'_, CallData>,
    import: &'static str,
    ptr: i32,
    len: i32,
) -> Result<String, wasmi::Error> {
    let bytes = read_guest(caller, import, ptr, len.max(0) as usize)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Register the whole `dss_env` import module on the linker (ABI doc §4).
///
/// # Panics
///
/// Only on duplicate registration — impossible by construction (each name is
/// registered exactly once below).
pub(crate) fn register_all(linker: &mut Linker<CallData>) {
    let l = linker;

    // --- Slot 1, tier B: MsgCallBack (`DSSCallBackRoutines.pas:95-99`). ---
    l.func_wrap(
        "dss_env",
        "msg_callback",
        |mut caller: Caller<'_, CallData>, ptr: i32, len: i32| -> Result<(), wasmi::Error> {
            let s = read_str(&mut caller, "msg_callback", ptr, len)?;
            caller.data_mut().effects.push(Effect::Msg(s));
            Ok(())
        },
    )
    .expect("duplicate dss_env import");

    // --- Slots 2-6, tier C: the owned AuxParser (`:103-147`). ---
    // Pascal `ParserIntValue` (`:111-116`): current token via `IntValue` =
    // `MakeInteger`. A conversion failure raises an FPC exception across the
    // Stdcall boundary upstream (UB-class); the defined port writes 0, like
    // the empty-token path.
    l.func_wrap(
        "dss_env",
        "get_int_value",
        |mut caller: Caller<'_, CallData>, ptr: i32| -> Result<(), wasmi::Error> {
            let data = caller.data_mut();
            let vars = std::mem::take(&mut data.parser_vars);
            let v = data.parser.make_integer(&vars).unwrap_or(0);
            caller.data_mut().parser_vars = vars;
            write_guest(&mut caller, "get_int_value", ptr, &v.to_le_bytes())
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `ParserDblValue` (`:120-125`); same failure note as above.
    l.func_wrap(
        "dss_env",
        "get_dbl_value",
        |mut caller: Caller<'_, CallData>, ptr: i32| -> Result<(), wasmi::Error> {
            let data = caller.data_mut();
            let vars = std::mem::take(&mut data.parser_vars);
            let v = data.parser.make_double(&vars).unwrap_or(0.0);
            caller.data_mut().parser_vars = vars;
            write_guest(&mut caller, "get_dbl_value", ptr, &v.to_le_bytes())
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `ParserStrValue` (`:128-136`): copies `CB_Param` — the value
    // captured by the last `NextParam` — NOT a fresh token. The upstream
    // `SetLength(CB_Param, Maxlen)` truncates the stored value when `Maxlen`
    // is shorter (reproduced); when longer it pads with *undefined* bytes
    // (UB-class, not reproduced — the defined observable is the original
    // value, NUL-terminated).
    l.func_wrap(
        "dss_env",
        "get_str_value",
        |mut caller: Caller<'_, CallData>, ptr: i32, maxlen: i32| -> Result<(), wasmi::Error> {
            let data = caller.data_mut();
            let keep = (maxlen.max(0) as usize).min(data.last_param_value.len());
            data.last_param_value.truncate(keep);
            let s = data.last_param_value.clone();
            str_lcopy(&mut caller, "get_str_value", ptr, maxlen, &s)?;
            Ok(())
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `ParserLoad` (`:103-108`).
    l.func_wrap(
        "dss_env",
        "load_parser",
        |mut caller: Caller<'_, CallData>, ptr: i32, len: i32| -> Result<(), wasmi::Error> {
            let s = read_str(&mut caller, "load_parser", ptr, len)?;
            caller.data_mut().parser.set_cmd_string(&s);
            Ok(())
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `ParserNextParam` (`:140-147`): advance, capture `CB_ParamName`
    // + `CB_Param`, copy the *name* to the guest, return the *value* length.
    l.func_wrap(
        "dss_env",
        "next_param",
        |mut caller: Caller<'_, CallData>,
         name_ptr: i32,
         maxlen: i32|
         -> Result<i32, wasmi::Error> {
            let data = caller.data_mut();
            let vars = std::mem::take(&mut data.parser_vars);
            let name = data.parser.next_param(&vars);
            let value = data.parser.make_string(&vars);
            let data = caller.data_mut();
            data.parser_vars = vars;
            data.last_param_value = value;
            let value_len = data.last_param_value.len() as i32;
            str_lcopy(&mut caller, "next_param", name_ptr, maxlen, &name)?;
            Ok(value_len)
        },
    )
    .expect("duplicate dss_env import");

    // --- Slot 7, tier C: DoDSSCommand — deferred to WP-WM.6 (ABI doc §4). ---
    l.func_wrap(
        "dss_env",
        "do_dss_command",
        |mut caller: Caller<'_, CallData>, _ptr: i32, _len: i32| -> Result<(), wasmi::Error> {
            Err(unsupported(&mut caller, "do_dss_command"))
        },
    )
    .expect("duplicate dss_env import");

    // --- Slots 8-29, tier A: pure reads from the context snapshot. ---
    // Pascal `GetActiveElementBusNamesCallBack` (`:157-190`).
    l.func_wrap(
        "dss_env",
        "get_active_element_bus_names",
        |mut caller: Caller<'_, CallData>,
         p1: i32,
         l1: i32,
         p2: i32,
         l2: i32|
         -> Result<(), wasmi::Error> {
            let (n1, n2) = caller.data().ctx.active_element_bus_names();
            let (n1, n2) = (n1.to_string(), n2.to_string());
            str_lcopy(&mut caller, "get_active_element_bus_names", p1, l1, &n1)?;
            str_lcopy(&mut caller, "get_active_element_bus_names", p2, l2, &n2)?;
            Ok(())
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `GetActiveElementVoltagesCallBack` (`:193-207`): NumVoltages is
    // in/out — buffer size in, `Min(Yorder, NumVoltages)` out; nil element ⇒
    // everything untouched.
    l.func_wrap(
        "dss_env",
        "get_active_element_voltages",
        |mut caller: Caller<'_, CallData>, num_ptr: i32, v_ptr: i32| -> Result<(), wasmi::Error> {
            const IMPORT: &str = "get_active_element_voltages";
            let Some(vs) = caller.data().ctx.active_element_voltages() else {
                return Ok(());
            };
            let avail = vs.len();
            let n = read_i32(&mut caller, IMPORT, num_ptr)?;
            let n = (n.max(0) as usize).min(avail);
            let bytes =
                cplx_slice_bytes(&caller.data().ctx.active_element_voltages().expect("Some")[..n]);
            write_guest(&mut caller, IMPORT, v_ptr, &bytes)?;
            write_guest(&mut caller, IMPORT, num_ptr, &(n as i32).to_le_bytes())
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `GetActiveElementCurrentsCallBack` (`:210-222`).
    l.func_wrap(
        "dss_env",
        "get_active_element_currents",
        |mut caller: Caller<'_, CallData>, num_ptr: i32, i_ptr: i32| -> Result<(), wasmi::Error> {
            const IMPORT: &str = "get_active_element_currents";
            let Some(cs) = caller.data().ctx.active_element_currents() else {
                return Ok(());
            };
            let avail = cs.len();
            let n = read_i32(&mut caller, IMPORT, num_ptr)?;
            let n = (n.max(0) as usize).min(avail);
            let bytes =
                cplx_slice_bytes(&caller.data().ctx.active_element_currents().expect("Some")[..n]);
            write_guest(&mut caller, IMPORT, i_ptr, &bytes)?;
            write_guest(&mut caller, IMPORT, num_ptr, &(n as i32).to_le_bytes())
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `GetActiveElementLossesCallBack` (`:225-234`): zeroed then
    // filled — always written.
    l.func_wrap(
        "dss_env",
        "get_active_element_losses",
        |mut caller: Caller<'_, CallData>,
         total_ptr: i32,
         load_ptr: i32,
         noload_ptr: i32|
         -> Result<(), wasmi::Error> {
            const IMPORT: &str = "get_active_element_losses";
            let [total, load, noload] = caller.data().ctx.active_element_losses();
            write_guest(&mut caller, IMPORT, total_ptr, &cplx_bytes(total))?;
            write_guest(&mut caller, IMPORT, load_ptr, &cplx_bytes(load))?;
            write_guest(&mut caller, IMPORT, noload_ptr, &cplx_bytes(noload))
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `GetActiveElementPowerCallBack` (`:237-244`).
    l.func_wrap(
        "dss_env",
        "get_active_element_power",
        |mut caller: Caller<'_, CallData>,
         terminal: i32,
         power_ptr: i32|
         -> Result<(), wasmi::Error> {
            let p = caller.data().ctx.active_element_power(terminal);
            write_guest(
                &mut caller,
                "get_active_element_power",
                power_ptr,
                &cplx_bytes(p),
            )
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `GetActiveElementNumCustCallBack` (`:247-264`).
    l.func_wrap(
        "dss_env",
        "get_active_element_num_cust",
        |mut caller: Caller<'_, CallData>,
         num_ptr: i32,
         total_ptr: i32|
         -> Result<(), wasmi::Error> {
            const IMPORT: &str = "get_active_element_num_cust";
            let (num, total) = caller.data().ctx.active_element_num_cust();
            write_guest(&mut caller, IMPORT, num_ptr, &num.to_le_bytes())?;
            write_guest(&mut caller, IMPORT, total_ptr, &total.to_le_bytes())
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `GetActiveElementNodeRefCallBack` (`:267-278`): nil ⇒ untouched.
    l.func_wrap(
        "dss_env",
        "get_active_element_node_ref",
        |mut caller: Caller<'_, CallData>, maxsize: i32, ptr: i32| -> Result<(), wasmi::Error> {
            let Some(refs) = caller.data().ctx.active_element_node_refs() else {
                return Ok(());
            };
            let n = refs.len().min(maxsize.max(0) as usize);
            let mut bytes = Vec::with_capacity(n * 4);
            for r in &caller.data().ctx.active_element_node_refs().expect("Some")[..n] {
                bytes.extend_from_slice(&r.to_le_bytes());
            }
            write_guest(&mut caller, "get_active_element_node_ref", ptr, &bytes)
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `GetActiveElementBusRefCallBack` (`:281-290`).
    l.func_wrap(
        "dss_env",
        "get_active_element_bus_ref",
        |caller: Caller<'_, CallData>, terminal: i32| -> i32 {
            caller.data().ctx.active_element_bus_ref(terminal)
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `GetActiveElementTerminalInfoCallBack` (`:293-304`): nil ⇒
    // untouched.
    l.func_wrap(
        "dss_env",
        "get_active_element_terminal_info",
        |mut caller: Caller<'_, CallData>,
         nt_ptr: i32,
         nc_ptr: i32,
         np_ptr: i32|
         -> Result<(), wasmi::Error> {
            const IMPORT: &str = "get_active_element_terminal_info";
            let Some((nt, nc, np)) = caller.data().ctx.active_element_terminal_info() else {
                return Ok(());
            };
            write_guest(&mut caller, IMPORT, nt_ptr, &nt.to_le_bytes())?;
            write_guest(&mut caller, IMPORT, nc_ptr, &nc.to_le_bytes())?;
            write_guest(&mut caller, IMPORT, np_ptr, &np.to_le_bytes())
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `GetPtrToSystemVarrayCallBack` (`:307-311`) → copy semantics
    // (ABI doc §4 row 17): write up to `max` node voltages, return the count.
    l.func_wrap(
        "dss_env",
        "get_node_voltages",
        |mut caller: Caller<'_, CallData>, dest: i32, max: i32| -> Result<i32, wasmi::Error> {
            let n = caller
                .data()
                .ctx
                .node_voltages()
                .len()
                .min(max.max(0) as usize);
            let bytes = cplx_slice_bytes(&caller.data().ctx.node_voltages()[..n]);
            write_guest(&mut caller, "get_node_voltages", dest, &bytes)?;
            Ok(n as i32)
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `GetActiveElementIndexCallBack` (`:315-325`).
    l.func_wrap(
        "dss_env",
        "get_active_element_index",
        |caller: Caller<'_, CallData>| -> i32 { caller.data().ctx.active_element_index() },
    )
    .expect("duplicate dss_env import");

    // Pascal `IsActiveElementEnabledCallBack` (`:328-338`); booleans are
    // i32 0/1 (ABI doc §4).
    l.func_wrap(
        "dss_env",
        "is_active_element_enabled",
        |caller: Caller<'_, CallData>| -> i32 {
            i32::from(caller.data().ctx.is_active_element_enabled())
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `IsBusCoordinateDefinedCallback` (`:341-346`).
    l.func_wrap(
        "dss_env",
        "is_bus_coordinate_defined",
        |caller: Caller<'_, CallData>, bus_ref: i32| -> i32 {
            i32::from(caller.data().ctx.is_bus_coordinate_defined(bus_ref))
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `GetBusCoordinateCallback` (`:348-357`): zeroed then filled —
    // always written.
    l.func_wrap(
        "dss_env",
        "get_bus_coordinate",
        |mut caller: Caller<'_, CallData>,
         bus_ref: i32,
         x_ptr: i32,
         y_ptr: i32|
         -> Result<(), wasmi::Error> {
            const IMPORT: &str = "get_bus_coordinate";
            let (x, y) = caller.data().ctx.bus_coordinate(bus_ref);
            write_guest(&mut caller, IMPORT, x_ptr, &x.to_le_bytes())?;
            write_guest(&mut caller, IMPORT, y_ptr, &y.to_le_bytes())
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `GetBuskVBaseCallback` (`:359-366`).
    l.func_wrap(
        "dss_env",
        "get_bus_kv_base",
        |caller: Caller<'_, CallData>, bus_ref: i32| -> f64 {
            caller.data().ctx.bus_kv_base(bus_ref)
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `GetBusDistFromMeterCallback` (`:368-375`).
    l.func_wrap(
        "dss_env",
        "get_bus_dist_from_meter",
        |caller: Caller<'_, CallData>, bus_ref: i32| -> f64 {
            caller.data().ctx.bus_dist_from_meter(bus_ref)
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `GetDynamicsStructCallBack` (`:377-383`) → copies the 52-byte
    // image (ABI doc §4 row 24); no circuit ⇒ untouched.
    l.func_wrap(
        "dss_env",
        "get_dynamics_rec",
        |mut caller: Caller<'_, CallData>, dest: i32| -> Result<(), wasmi::Error> {
            let Some(rec) = caller.data().ctx.dynamics_rec() else {
                return Ok(());
            };
            write_guest(&mut caller, "get_dynamics_rec", dest, &rec.to_bytes())
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `GetStepSizeCallBack` (`:385-392`).
    l.func_wrap(
        "dss_env",
        "get_step_size",
        |caller: Caller<'_, CallData>| -> f64 { caller.data().ctx.step_size() },
    )
    .expect("duplicate dss_env import");

    // Pascal `GetTimeSecCallBack` (`:394-400`).
    l.func_wrap(
        "dss_env",
        "get_time_sec",
        |caller: Caller<'_, CallData>| -> f64 { caller.data().ctx.time_sec() },
    )
    .expect("duplicate dss_env import");

    // Pascal `GetTimeHrCallBack` (`:402-408`).
    l.func_wrap(
        "dss_env",
        "get_time_hr",
        |caller: Caller<'_, CallData>| -> f64 { caller.data().ctx.time_hr() },
    )
    .expect("duplicate dss_env import");

    // Pascal `GetPublicDataPtrCallBack` (`:411-421`) → copy semantics (ABI
    // doc §4 row 28): write up to `max` bytes of the public-data image,
    // return the copied byte count.
    l.func_wrap(
        "dss_env",
        "get_public_data",
        |mut caller: Caller<'_, CallData>, dest: i32, max: i32| -> Result<i32, wasmi::Error> {
            let n = caller
                .data()
                .ctx
                .public_data()
                .len()
                .min(max.max(0) as usize);
            let bytes = caller.data().ctx.public_data()[..n].to_vec();
            write_guest(&mut caller, "get_public_data", dest, &bytes)?;
            Ok(n as i32)
        },
    )
    .expect("duplicate dss_env import");

    // Pascal `GetActiveElementNameCallBack` (`:423-437`): nil ⇒ Result 0,
    // buffer untouched; else StrLCopy + truncated length.
    l.func_wrap(
        "dss_env",
        "get_active_element_name",
        |mut caller: Caller<'_, CallData>, ptr: i32, maxlen: i32| -> Result<i32, wasmi::Error> {
            let Some(name) = caller.data().ctx.active_element_name() else {
                return Ok(0);
            };
            let name = name.to_string();
            str_lcopy(&mut caller, "get_active_element_name", ptr, maxlen, &name)
        },
    )
    .expect("duplicate dss_env import");

    // --- Slot 30: GetActiveElementPtr — not importable (ABI doc §4 row 30):
    // a raw host pointer has no wasm meaning; models needing element
    // internals use `get_public_data`. Loud error when called. ---
    l.func_wrap(
        "dss_env",
        "get_active_element_ptr",
        |mut caller: Caller<'_, CallData>| -> Result<i32, wasmi::Error> {
            Err(unsupported(&mut caller, "get_active_element_ptr"))
        },
    )
    .expect("duplicate dss_env import");

    // --- Slot 31, tier B: ControlQueuePush (`:444-447`) — queued, drained in
    // order after the call returns; provisional handles `seed`, `seed+1`, …
    // reproduce the Pascal queue-handle sequence (see
    // `Callbacks::control_queue_next_handle`). ---
    l.func_wrap(
        "dss_env",
        "control_queue_push",
        |mut caller: Caller<'_, CallData>, hour: i32, sec: f64, code: i32, proxy_hdl: i32| -> i32 {
            let data = caller.data_mut();
            let queued = data
                .effects
                .iter()
                .filter(|e| matches!(e, Effect::ControlQueuePush { .. }))
                .count() as i32;
            let handle = data.ctx.control_queue_next_handle() + queued;
            data.effects.push(Effect::ControlQueuePush {
                hour,
                sec,
                code,
                proxy_hdl,
                handle,
            });
            handle
        },
    )
    .expect("duplicate dss_env import");

    // --- Slot 32, tier C: GetResultStr — paired with `do_dss_command`,
    // deferred to WP-WM.6 (ABI doc §4 row 32). ---
    l.func_wrap(
        "dss_env",
        "get_result_str",
        |mut caller: Caller<'_, CallData>, _ptr: i32, _maxlen: i32| -> Result<(), wasmi::Error> {
            Err(unsupported(&mut caller, "get_result_str"))
        },
    )
    .expect("duplicate dss_env import");
}

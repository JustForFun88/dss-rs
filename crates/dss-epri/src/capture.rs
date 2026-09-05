//! `CaseResult` assembly — a byte-for-byte port of
//! `tools/oracle/oracle_server.py::run_case` (+ the `capture_*` helpers it shares
//! with `tools/golden/gen_checkpoints.py`) against the raw r4133 DLL.
//!
//! The response is JSON-shape-identical to the retired Oddie oracle's
//! (`CaseResult { node_order, n_steps, checkpoints, autoadd_log }`), so the Rust
//! gate's `serde` deserialize accepts it unchanged (bit-diff-proven against the
//! Python path by `xcheck_bridge.py`, itself retired with that stack in Phase
//! E). Read order within a step matches `oracle_server` exactly (notably
//! Powers-before-Currents in the element capture — the harmonics stale-`Iterminal`
//! ordering, CLAUDE.md).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::dss::{Engine, EngineError};
use crate::guard::CorpusGuard;

/// Retry a non-converged case in-process up to this many times
/// (`oracle_server._RUN_ATTEMPTS`): absorbs the engine's occasional
/// fresh-process convergence misfire without masking a real non-convergence.
const RUN_ATTEMPTS: usize = 3;

// ---------------------------------------------------------------------------
// Request (deserialized from the line-JSON `run` message).
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RunRequest {
    pub case_path: String,
    #[serde(default)]
    pub post: Vec<String>,
    #[serde(default = "one")]
    pub n_steps: usize,
    #[serde(default)]
    pub selected_elements: Vec<String>,
    #[serde(default)]
    pub check_meters_monitors: bool,
    #[serde(default)]
    pub probes: Vec<ProbeSpec>,
    #[serde(default)]
    pub variables: Vec<String>,
    #[serde(default)]
    pub eventlog: bool,
    #[serde(default)]
    pub ctrlqueue: bool,
    /// G1.4a `compare_bus`: capture every bus's voltage surface
    /// ([`capture_all_buses`]) plus the checkpoint-level `AllBusVmagPu`.
    #[serde(default)]
    pub buses: bool,
    /// G1.5 `compare_zsc`: append the six short-circuit arms
    /// (`Zsc1`/`Zsc0`/`ZscMatrix`/`YscMatrix`/`Isc`/`Voc`) to the ONE per-bus
    /// walk [`Self::buses`] drives — never a second `SetActiveBus` pass.
    /// Requires [`Self::buses`]: without it the walk does not run at all, so
    /// [`run_case`] refuses the malformed request instead of silently shipping
    /// an empty surface (the gate asserts the implication one level up, in
    /// `corpus_gate::engines::build_run_request`).
    #[serde(default)]
    pub zsc: bool,
    #[serde(default)]
    pub all_properties: bool,
    #[serde(default)]
    pub global_result: bool,
    #[serde(default)]
    pub autoadd_log: bool,
    #[serde(default)]
    pub warn_and_continue: bool,
    // `full_csc` is accepted but ignored: the gate always requests it (true) and
    // the r4133 CSC export is proven solution-neutral by the smoke self-test.
    #[serde(default)]
    #[allow(dead_code)]
    pub full_csc: Option<bool>,
    #[serde(default)]
    #[allow(dead_code)]
    pub cmd: Option<String>,
}

fn one() -> usize {
    1
}

#[derive(Debug, Deserialize)]
pub struct ProbeSpec {
    pub element: String,
    #[serde(default)]
    pub props: Vec<String>,
}

// ---------------------------------------------------------------------------
// Response shapes (serialized — must match oracle_server.py field-for-field).
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct CaseResult {
    node_order: Vec<String>,
    n_steps: usize,
    checkpoints: Vec<Checkpoint>,
    autoadd_log: Option<String>,
}

#[derive(Serialize)]
struct Checkpoint {
    dbl_hour: f64,
    iterations: i32,
    converged: bool,
    v_re: Vec<f64>,
    v_im: Vec<f64>,
    y: Option<YMat>,
    y_fingerprint: YFingerprint,
    yprims: Vec<YPrim>,
    elements: Vec<ElementCap>,
    injection: Injection,
    transformers: BTreeMap<String, Vec<f64>>,
    regcontrols: BTreeMap<String, i32>,
    capacitors: BTreeMap<String, Vec<i32>>,
    monitors: Vec<MonitorCap>,
    meters: Vec<MeterCap>,
    probes: Vec<ProbeCap>,
    variables: Vec<VariablesCap>,
    eventlog: Vec<String>,
    ctrlqueue: Vec<String>,
    buses: Vec<BusCap>,
    all_bus_vmag_pu: Vec<f64>,
    all_properties: Vec<PropsCap>,
    global_result: String,
}

#[derive(Serialize)]
struct YMat {
    n: usize,
    rows: Vec<i32>,
    cols: Vec<i32>,
    re: Vec<f64>,
    im: Vec<f64>,
}

#[derive(Serialize)]
struct YFingerprint {
    nnz: usize,
    frob: f64,
    tr_re: f64,
    tr_im: f64,
    maxdiag: f64,
}

#[derive(Serialize)]
struct YPrim {
    name: String,
    yorder: usize,
    re: Vec<f64>,
    im: Vec<f64>,
}

#[derive(Serialize)]
struct ElementCap {
    name: String,
    i_re: Vec<f64>,
    i_im: Vec<f64>,
    p_kw: Vec<f64>,
    p_kvar: Vec<f64>,
    loss_w: Vec<f64>,
}

#[derive(Serialize)]
struct Injection {
    re: Vec<f64>,
    im: Vec<f64>,
}

#[derive(Serialize)]
struct MonitorCap {
    name: String,
    header: Vec<String>,
    sample_count: i64,
    channels: Vec<Vec<f64>>,
}

#[derive(Serialize)]
struct MeterCap {
    name: String,
    register_names: Vec<String>,
    register_values: Vec<f64>,
    n_branches: usize,
    n_ends: usize,
    n_pce: usize,
    branches: Vec<String>,
    ends: Vec<String>,
    pce: Vec<String>,
}

#[derive(Serialize)]
struct ProbeCap {
    element: String,
    prop: String,
    value: String,
}

#[derive(Serialize)]
struct VariablesCap {
    name: String,
    var_names: Vec<String>,
    values: Vec<f64>,
}

/// One bus's captured voltage surface — the `compare_bus` wire shape
/// (GOLDEN_REBASE_PLAN.md WP-G1 sub-step G1.4a), serialized field-for-field as
/// `tools/oracle/oracle_server.py::capture_all_buses` emits it and
/// `corpus_gate::engines::BusCap` deserializes it.
///
/// Parity target: fastdss' `IBus._columns` (`origin/fastdss` `dss/IBus.py:19-53`,
/// reached through `save_state`'s `ActiveBus`, `tests/save_outputs.py:351`).
/// The four surfaces below are the ones both gating channels compute with the
/// identical algorithm; the bus quantities that diverge
/// (`SeqVoltages`/`CplxSeqVoltages`, `VLL`/`puVLL` — the latter also hang this
/// channel, `DBus.pas:575-583`) belong to G1.4c and are deliberately not read
/// (coordinator decision D8).
///
/// All three VOLTAGE arrays are `2 * nodes.len()` doubles in ONE order --
/// **ascending node number** — and never the bus's internal insertion order:
/// see [`crate::modes::BUS_NODES`] for the `FindIdx` walk all four arms share.
/// The six G1.5 short-circuit arrays below are the other convention — the bus's
/// INTERNAL node index — and are captured only when the request sets `zsc`
/// ([`RunRequest::zsc`]); see [`capture_all_buses`].
#[derive(Serialize)]
struct BusCap {
    /// `Circuit.AllBusNames` entry i, re-asserted as the active bus by
    /// `SetActiveBus`'s returned index (`DCircuit.pas:439`, `:247-250`).
    name: String,
    /// `Bus.kVBase` in kV (`BUSF(0)`). Both engines take
    /// `BaseFactor = 1000 * kVBase` when positive, else `1.0`
    /// (`DBus.pas:413-414` == `CAPI_Alt.pas:2262-2265`).
    kv_base: f64,
    /// `Bus.Nodes` — node numbers, ascending (`BUSV(2)`, `DBus.pas:319-345`).
    nodes: Vec<i32>,
    /// `Bus.puVoltages` — `NodeV[GetRef]/BaseFactor`, interleaved `(re, im)`
    /// (`BUSV(5)`, `DBus.pas:399-430` == `CAPI_Alt.pas:2251-2280`).
    pu_voltages: Vec<f64>,
    /// `Bus.VMagAngle` — interleaved `(magnitude V, angle deg)`
    /// (`BUSV(13)`, `DBus.pas:659-689` == `CAPI_Alt.pas:2573-2597`).
    vmag_angle: Vec<f64>,
    /// `Bus.puVMagAngle` — the same pairs with only the magnitude divided by
    /// `BaseFactor` (`BUSV(14)`, `DBus.pas:690-723` == `CAPI_Alt.pas:2540-2571`).
    pu_vmag_angle: Vec<f64>,
    /// `Bus.Zsc1` = `Zs - Zm`, ONE complex = 2 doubles, always (`BUSV(7)`,
    /// `DBus.pas:461-474` == `CAPI_Alt.pas:2294-2303`; both write the
    /// 1-element array unconditionally). `cZERO` while `Zsc` is unassigned
    /// (`Common/Bus.pas:222-229` == capi `:225-232`). `AvgOffDiagonal` divides
    /// only `If Ntimes > 0` (`Shared/Ucmatrix.pas:369-383` == capi `:372-387`),
    /// so a 1-node bus has `Zm = 0` and `zsc1 == zsc0 == zsc[0]`.
    zsc1: Vec<f64>,
    /// `Bus.Zsc0` = `Zs + 2*Zm`, same shape and guard (`BUSV(8)`,
    /// `DBus.pas:476-489` == `CAPI_Alt.pas:2283-2292`).
    zsc0: Vec<f64>,
    /// `Bus.ZscMatrix` — row-major (`i` outer, `j` inner) `2*n*n` doubles
    /// (`BUSV(6)`, `DBus.pas:431-459` == `CAPI_Alt.pas:2305-2334`), or the
    /// [`R4133_SC_SENTINEL_LEN`] sentinel while the bus has no matrix.
    zsc: Vec<f64>,
    /// `Bus.YscMatrix` — the same shape, `Ysc = Zsc^-1` (`BUSV(9)`,
    /// `DBus.pas:491-518` == `CAPI_Alt.pas:2336-2365`).
    ysc: Vec<f64>,
    /// `Bus.Isc` — `BusCurrent`, `2*n` doubles (`BUSV(4)`, `DBus.pas:374-397`
    /// == `CAPI_Alt.pas:2202-2224`).
    isc: Vec<f64>,
    /// `Bus.Voc` — `VBus`, `2*n` doubles (`BUSV(3)`, `DBus.pas:351-372` ==
    /// `CAPI_Alt.pas:2227-2249`). Refreshed by the fault study AND by
    /// `BuildYMatrix` under `PreserveNodeVoltages` (`Ymatrix.pas:170`), so it
    /// is live on harmonics/dynamics decks too.
    voc: Vec<f64>,
}

/// What THIS transport publishes for `Bus.ZscMatrix`/`Bus.YscMatrix` — and for
/// `Bus.Isc`/`Bus.Voc` on a 0-node bus — when the underlying pointer is nil:
/// the `setlength(myCmplxArray, 1); myCmplxArray[0] := CZero` prelude every
/// `BUSV` arm opens with, i.e. **2** doubles (`DDLL/DBus.pas:433-434` for
/// `Zsc`, `:493-494` for `Ysc`, `:353-354` for `Voc`, `:376-377` for `Isc`).
///
/// The capi transport publishes ONE double there instead (`DefaultResult`,
/// `CAPI/CAPI_Utils.pas:212-221` under `DSS_CAPI_COM_DEFAULTS`) — and, for
/// `Isc`/`Voc` at a 0-node bus, ZERO doubles, because capi's
/// `TDSSBus.AllocateBusState` uses `AllocMem` (`Common/Bus.pas:250-256`),
/// whose 0-byte block is non-nil, while r4133's `Reallocmem(VBus, 0)`
/// (`Common/Bus.pas:246-260`) frees the pointer. Measured on
/// `Test/REACTORTest.DSS` / `Test/Source012Test.dss` (`loadbus2`): capi
/// `(isc, voc) = (0, 0)` vs r4133 `(2, 2)`. The comparator normalizes both
/// sentinel shapes to "no matrix" / "no nodes"; they are never compared as
/// values.
const R4133_SC_SENTINEL_LEN: usize = 2;

/// One element's every-property dump (§2.2 all-properties parity — a **gating**
/// capture since R4133_PROPS RP4.1, 2026-09-03; report tooling only before it).
/// Serializes to the exact shape
/// `oracle_server.capture_all_properties` emits and `harness::PropsCap`
/// deserializes: `{"element": name, "props": [[prop, value], ...]}` in
/// `AllPropertyNames` (property-index) order.
#[derive(Serialize)]
pub struct PropsCap {
    pub element: String,
    pub props: Vec<(String, String)>,
}

// ---------------------------------------------------------------------------
// The run.
// ---------------------------------------------------------------------------

/// Compile one deck, solve `n_steps` times, capture the full per-step model.
pub fn run_case(engine: &Engine, req: &RunRequest) -> Result<CaseResult, EngineError> {
    let warn = req.warn_and_continue;
    let star = req.selected_elements == ["*"];

    let _guard = CorpusGuard::new(&req.case_path);

    let mut node_order: Vec<String> = Vec::new();
    let mut checkpoints: Vec<Checkpoint> = Vec::new();

    for attempt in 1..=RUN_ATTEMPTS {
        node_order.clear();
        checkpoints = Vec::with_capacity(req.n_steps);
        engine.clear()?;
        engine.compile(&req.case_path, warn)?;
        for c in &req.post {
            engine.post(c)?;
        }
        for step in 0..req.n_steps {
            let reply = engine.solve(warn)?;
            let global_result = if req.global_result {
                reply
            } else {
                String::new()
            };

            // selected_elements=["*"] -> every YPrim-bearing element (rebuilt each
            // solve so a deck that adds an element mid-solve is covered).
            let sel: Vec<String> = if star {
                let mut s = Vec::new();
                for nm in engine.all_element_names() {
                    engine.set_active_element(&nm);
                    let flat = engine.element_yprim();
                    let len = flat.len();
                    if len > 0 && is_square_yprim(len) {
                        s.push(nm);
                    }
                }
                engine.assert_clean("sel build")?;
                s
            } else {
                req.selected_elements.clone()
            };

            if node_order.is_empty() {
                node_order = engine.ynode_order();
            }
            let varray = engine.ynode_varray();
            let (v_re, v_im) = deinterleave(&varray);
            engine.assert_clean("voltages")?;

            let (transformers, regcontrols, capacitors) = capture_discrete(engine)?;

            let dbl_hour = engine.dbl_hour();
            let iterations = engine.iterations();
            let converged = engine.converged();

            let ycsc = engine.y_csc()?;
            let y = Some(build_ymat(&ycsc));
            let y_fingerprint = build_fingerprint(&ycsc);

            let mut yprims = Vec::with_capacity(sel.len());
            for nm in &sel {
                engine.set_active_element(nm);
                let flat = engine.element_yprim();
                yprims.push(build_yprim(nm, &flat));
            }
            engine.assert_clean("yprims")?;

            let elements = capture_all_elements(engine, warn)?;

            let inj = engine.injection_raw(engine.num_nodes());
            let injection = capture_injection(&inj);
            engine.assert_clean("injection")?;

            let monitors = if req.check_meters_monitors {
                capture_monitors(engine)?
            } else {
                Vec::new()
            };
            let meters = if req.check_meters_monitors {
                capture_meters(engine)?
            } else {
                Vec::new()
            };

            let probes = capture_probes(engine, &req.probes)?;
            let variables = capture_variables(engine, &req.variables)?;
            let eventlog = if req.eventlog {
                capture_eventlog(engine)?
            } else {
                Vec::new()
            };
            let ctrlqueue = if req.ctrlqueue {
                capture_ctrlqueue(engine)
            } else {
                Vec::new()
            };
            let (buses, all_bus_vmag_pu) = if req.buses {
                let buses = capture_all_buses(engine, req.zsc)?;
                let all_bus_vmag_pu = capture_all_bus_vmag_pu(engine)?;
                let nodes: usize = buses.iter().map(|b| b.nodes.len()).sum();
                if all_bus_vmag_pu.len() != nodes {
                    return Err(EngineError::Other(format!(
                        "bus capture: AllBusVmagPu has {} values but the per-bus walk saw {nodes} \
                         nodes over {} buses",
                        all_bus_vmag_pu.len(),
                        buses.len()
                    )));
                }
                (buses, all_bus_vmag_pu)
            } else if req.zsc {
                // G1.5: the six SC arms ride the per-bus walk above, so asking
                // for them without the bus surface would ship nothing at all.
                // Refuse loudly (the gate asserts the same implication in
                // `corpus_gate::engines::build_run_request`).
                return Err(EngineError::Other(
                    "request asks for the bus short-circuit surface (zsc) without the bus \
                     surface it is appended to: the six SC arms share the one per-bus walk \
                     (GOLDEN_REBASE_PLAN.md WP-G1 G1.5 section 2.a)"
                        .into(),
                ));
            } else {
                (Vec::new(), Vec::new())
            };
            // Read LAST (after every other capture), like `oracle_server.run_case`:
            // the `? name.Like`/`? name.prop` sweep perturbs the active-element
            // cursor, so it must not run before any other read (§2.2).
            let all_properties = if req.all_properties {
                capture_all_properties(engine)?
            } else {
                Vec::new()
            };

            let _ = step;
            checkpoints.push(Checkpoint {
                dbl_hour,
                iterations,
                converged,
                v_re,
                v_im,
                y,
                y_fingerprint,
                yprims,
                elements,
                injection,
                transformers,
                regcontrols,
                capacitors,
                monitors,
                meters,
                probes,
                variables,
                eventlog,
                ctrlqueue,
                buses,
                all_bus_vmag_pu,
                all_properties,
                global_result,
            });
        }

        let bad: Vec<usize> = checkpoints
            .iter()
            .enumerate()
            .filter(|(_, cp)| !cp.converged)
            .map(|(i, _)| i)
            .collect();
        if bad.is_empty() {
            break;
        }
        eprintln!(
            "epri-worker retry: {} attempt {attempt}/{RUN_ATTEMPTS} non-converged step(s) {bad:?}",
            req.case_path
        );
    }

    let autoadd_log = if req.autoadd_log {
        read_autoadd_log(engine, &req.case_path)
    } else {
        None
    };

    Ok(CaseResult {
        node_order,
        n_steps: req.n_steps,
        checkpoints,
        autoadd_log,
    })
}

/// `len` (flat re/im floats) is a square YPrim: `len == 2 * yorder^2`.
fn is_square_yprim(len: usize) -> bool {
    let half = len / 2;
    let n = (half as f64).sqrt() as usize;
    // check n and n+1 to avoid float floor error
    for cand in [n.saturating_sub(1), n, n + 1] {
        if cand > 0 && 2 * cand * cand == len {
            return true;
        }
    }
    false
}

fn deinterleave(flat: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let re = flat.iter().step_by(2).copied().collect();
    let im = flat.iter().skip(1).step_by(2).copied().collect();
    (re, im)
}

fn build_ymat(y: &crate::dss::Ycsc) -> YMat {
    let mut rows = Vec::with_capacity(y.row_idx.len());
    let mut cols = Vec::with_capacity(y.row_idx.len());
    let mut re = Vec::with_capacity(y.row_idx.len());
    let mut im = Vec::with_capacity(y.row_idx.len());
    for col in 0..y.n {
        let start = y.col_ptr[col] as usize;
        let end = y.col_ptr[col + 1] as usize;
        for k in start..end {
            rows.push(y.row_idx[k]);
            cols.push(col as i32);
            re.push(y.vals[2 * k]);
            im.push(y.vals[2 * k + 1]);
        }
    }
    YMat {
        n: y.n,
        rows,
        cols,
        re,
        im,
    }
}

/// `capture_fingerprint` (floor 1e-9): counts, Frobenius norm, complex trace,
/// max |diagonal|. Arithmetic matches the Python: `abs(x)` = `hypot(re, im)`,
/// summed in CSC storage order, `frob = sqrt(sum(abs^2))`.
fn build_fingerprint(y: &crate::dss::Ycsc) -> YFingerprint {
    const FLOOR: f64 = 1e-9;
    let mut nnz: usize = 0;
    let mut frob_acc: f64 = 0.0;
    let mut tr_re: f64 = 0.0;
    let mut tr_im: f64 = 0.0;
    let mut maxdiag: f64 = 0.0;
    for col in 0..y.n {
        let start = y.col_ptr[col] as usize;
        let end = y.col_ptr[col + 1] as usize;
        for k in start..end {
            let re = y.vals[2 * k];
            let im = y.vals[2 * k + 1];
            let mag = re.hypot(im);
            if mag > FLOOR {
                nnz += 1;
            }
            frob_acc += mag * mag;
            if y.row_idx[k] as usize == col {
                tr_re += re;
                tr_im += im;
                if mag > maxdiag {
                    maxdiag = mag;
                }
            }
        }
    }
    YFingerprint {
        nnz,
        frob: frob_acc.sqrt(),
        tr_re,
        tr_im,
        maxdiag,
    }
}

fn build_yprim(name: &str, flat: &[f64]) -> YPrim {
    let (re, im) = deinterleave(flat);
    let yorder = (re.len() as f64).sqrt().round() as usize;
    YPrim {
        name: name.to_string(),
        yorder,
        re,
        im,
    }
}

fn capture_injection(flat: &[f64]) -> Injection {
    // slot 0 = ground; drop it.
    let (re, im) = deinterleave(flat);
    Injection {
        re: re.into_iter().skip(1).collect(),
        im: im.into_iter().skip(1).collect(),
    }
}

/// `capture_all_elements`: every element's terminal currents/powers/losses, read
/// Powers-then-Currents-then-Losses with the user-model retry.
fn capture_all_elements(engine: &Engine, warn: bool) -> Result<Vec<ElementCap>, EngineError> {
    let names = engine.all_element_names();
    let mut out = Vec::with_capacity(names.len());
    for name in names {
        engine.set_active_element(&name);
        let (powers, currents, losses) = engine.element_pcl(warn, &format!("element {name}"))?;
        let (i_re, i_im) = deinterleave(&currents);
        let (p_kw, p_kvar) = deinterleave(&powers);
        let loss_w = vec![
            losses.first().copied().unwrap_or(0.0),
            losses.get(1).copied().unwrap_or(0.0),
        ];
        out.push(ElementCap {
            name,
            i_re,
            i_im,
            p_kw,
            p_kvar,
            loss_w,
        });
    }
    Ok(out)
}

/// Per-step discrete control state: transformer winding taps, RegControl tap
/// numbers, capacitor step states.
type DiscreteState = (
    BTreeMap<String, Vec<f64>>,
    BTreeMap<String, i32>,
    BTreeMap<String, Vec<i32>>,
);

fn capture_discrete(engine: &Engine) -> Result<DiscreteState, EngineError> {
    let mut transformers = BTreeMap::new();
    let mut has = engine.transformers_first();
    while has {
        let nw = engine.transformer_num_windings();
        let mut taps = Vec::with_capacity(nw.max(0) as usize);
        for w in 1..=nw {
            engine.transformer_set_wdg(w);
            taps.push(engine.transformer_tap());
        }
        transformers.insert(engine.transformer_name(), taps);
        has = engine.transformers_next();
    }
    let mut regcontrols = BTreeMap::new();
    let mut has = engine.regcontrols_first();
    while has {
        regcontrols.insert(engine.regcontrol_name(), engine.regcontrol_tap_number());
        has = engine.regcontrols_next();
    }
    let mut capacitors = BTreeMap::new();
    let mut has = engine.capacitors_first();
    while has {
        capacitors.insert(engine.capacitor_name(), engine.capacitor_states());
        has = engine.capacitors_next();
    }
    engine.assert_clean("discrete")?;
    Ok((transformers, regcontrols, capacitors))
}

fn capture_monitors(engine: &Engine) -> Result<Vec<MonitorCap>, EngineError> {
    let mut out = Vec::new();
    let mut has = engine.monitors_first();
    while has {
        let nch = engine.monitor_num_channels();
        let channels: Vec<Vec<f64>> = (1..=nch).map(|c| engine.monitor_channel(c)).collect();
        out.push(MonitorCap {
            name: engine.monitor_name(),
            header: engine.monitor_header(),
            sample_count: engine.monitor_sample_count() as i64,
            channels,
        });
        has = engine.monitors_next();
    }
    engine.assert_clean("monitors")?;
    Ok(out)
}

fn capture_meters(engine: &Engine) -> Result<Vec<MeterCap>, EngineError> {
    let mut out = Vec::new();
    let mut has = engine.meters_first();
    while has {
        let branches = lst(engine.meter_all_branches_in_zone());
        let ends = lst(engine.meter_all_end_elements());
        let pce = lst(engine.meter_zone_pce());
        out.push(MeterCap {
            name: engine.meter_name(),
            register_names: engine.meter_register_names(),
            register_values: engine.meter_register_values(),
            n_branches: branches.len(),
            n_ends: ends.len(),
            n_pce: pce.len(),
            branches,
            ends,
            pce,
        });
        has = engine.meters_next();
    }
    engine.assert_clean("meters")?;
    Ok(out)
}

/// `_lst`: strip whitespace, drop empties; an all-`NONE` placeholder list -> [].
///
/// The empty-zone placeholder is compared case-insensitively: the raw DDLL writes
/// `'None'` (mixed case, `DMeters.pas`) but the Oddie backend normalizes the empty
/// string-array to the AltDSS `'NONE'` DefaultResult, and `oracle_server._lst`
/// filters that. Matching the *filtered* result (both `[]`) is the byte-for-byte
/// contract — an element name is never "none".
fn lst(v: Vec<String>) -> Vec<String> {
    let xs: Vec<String> = v
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if xs.len() == 1 && xs[0].eq_ignore_ascii_case("NONE") {
        Vec::new()
    } else {
        xs
    }
}

fn capture_probes(engine: &Engine, specs: &[ProbeSpec]) -> Result<Vec<ProbeCap>, EngineError> {
    let mut out = Vec::new();
    for spec in specs {
        for p in &spec.props {
            let value = engine.raw_command(&format!("? {}.{}", spec.element, p));
            let (errno, desc) = engine.poll_error();
            if errno != 0 {
                return Err(EngineError::Dss {
                    errno,
                    desc,
                    ctx: format!("probe {}.{}", spec.element, p),
                });
            }
            out.push(ProbeCap {
                element: spec.element.clone(),
                prop: p.clone(),
                value,
            });
        }
    }
    Ok(out)
}

/// Every circuit element's every property value (§2.2 all-properties parity),
/// a byte-for-byte port of `oracle_server.capture_all_properties`: for each
/// `AllElementNames` entry, activate it with `? name.Like` (the WPG.1-safe query
/// path — activates `DSS_OBJECT`s too, unlike `SetActiveElement`), read the
/// class's `AllPropertyNames` off the now-active object (`DSSElementV` mode 0),
/// then read each value via `? name.prop` in property-index order.
///
/// **Gating, since R4133_PROPS RP4.1 (2026-09-03).** This capture was
/// report-tooling only while property parity was pinned to capi_v0145 and the
/// scheduler masked `all_properties` off the r4133 request; RP4.1 removed both
/// masks, so what this function returns is now value-compared against the port
/// for every live non-`large` r4133-gating case. Any non-zero errno on a read
/// escalates, exactly like [`capture_probes`] (matching dss-python's
/// raise-on-error).
fn capture_all_properties(engine: &Engine) -> Result<Vec<PropsCap>, EngineError> {
    let mut out = Vec::new();
    for name in engine.all_element_names() {
        // Activate via the query path (side-effect: sets ActiveDSSObject), then
        // read the property-name list off the active object.
        engine.raw_command(&format!("? {name}.Like"));
        let (errno, desc) = engine.poll_error();
        if errno != 0 {
            return Err(EngineError::Dss {
                errno,
                desc,
                ctx: format!("all_properties activate {name}"),
            });
        }
        let prop_names = engine.element_all_property_names();
        let mut props = Vec::with_capacity(prop_names.len());
        for p in &prop_names {
            let value = engine.raw_command(&format!("? {name}.{p}"));
            let (errno, desc) = engine.poll_error();
            if errno != 0 {
                return Err(EngineError::Dss {
                    errno,
                    desc,
                    ctx: format!("all_properties {name}.{p}"),
                });
            }
            props.push((p.clone(), value));
        }
        out.push(PropsCap {
            element: name,
            props,
        });
    }
    Ok(out)
}

/// Oracle-free smoke hook (§2.4): dump every element's every property for the
/// currently-compiled circuit, so `smoke.rs` can prove the `DSSElementV`
/// enumeration + `? name.prop` value read round-trips without an oracle.
pub fn all_properties_dump(engine: &Engine) -> Result<Vec<PropsCap>, EngineError> {
    capture_all_properties(engine)
}

/// Every bus's node set, kV base and the three per-node voltage surfaces — the
/// r4133 half of the `compare_bus` capture, a field-for-field port of
/// `oracle_server.capture_all_buses` over the typed mode accessors
/// ([`crate::modes`] rows `Circuit.AllBusNames`, `Bus.Nodes`, `Bus.puVoltages`,
/// `Bus.VMagAngle`, `Bus.puVMagAngle`; `Bus.kVBase` is `BUSF(0)`).
///
/// Walked in `BusList` order, which `SetActiveBus`'s returned 0-based index
/// (`DCircuit.pas:247-250`, `ActiveBusIndex - 1`) re-asserts per bus: a failed
/// lookup leaves `ActiveBusIndex` at 0 (`Common/DSSGlobals.pas:739-757`) and
/// would otherwise attribute the previous bus's voltages to this one.
///
/// Capture-order class **C, order-free** (GOLDEN_REBASE_PLAN.md §1.1(a),
/// coordinator decision D3): every arm reads `Solution.NodeV` directly and
/// touches neither `ComputeIterminal` nor `ActiveCktElement` — only
/// `ActiveBusIndex` moves. The per-bus read order below matches the capi
/// transport's and is a contract, not a staleness hazard.
///
/// Shapes are asserted, never assumed: `2 * len(nodes)` per value array (a
/// 0-node bus — 2 in the corpus — yields empty arrays and passes at `0 == 0`),
/// and the node numbers must come back strictly ascending, which is what makes
/// this capture comparable to the port's sorted view.
///
/// `want_sc` (G1.5, request field `zsc`) appends the six short-circuit arms to
/// THIS walk — never a second `SetActiveBus` pass — in the fixed order
/// `zsc1, zsc0, zsc, ysc, isc, voc`, matching `oracle_server.capture_all_buses`
/// arm for arm; the fields are always serialized, empty when it is off. They
/// are group C as well: `BUSV` 3/4/6/7/8/9 read `Zsc`/`Ysc`/`VBus`/
/// `BusCurrent` off the bus object with no `ComputeIterminal` and no
/// `ActiveCktElement` (all six are [`crate::modes::ModeEffect::Pure`]).
///
/// Unlike the three voltage surfaces, every SC array is indexed by the bus's
/// INTERNAL (insertion) node index: `Zsc`/`Ysc` are built column by column over
/// the bus's internal index (`GetRef(i)` in `ComputeYsc`,
/// `Common/SolutionAlgs.pas:800-832`; `pBus.RefNo[i]` in capi `:788-816`) and `VBus`/`BusCurrent` are stored per internal index. Their own
/// shapes are asserted per arm against [`R4133_SC_SENTINEL_LEN`] — a violation
/// fails the case loudly instead of shipping a short row the comparator would
/// misread as a value divergence.
fn capture_all_buses(engine: &Engine, want_sc: bool) -> Result<Vec<BusCap>, EngineError> {
    let names = engine.circuit_all_bus_names()?;
    let mut out = Vec::with_capacity(names.len());
    for (i, name) in names.iter().enumerate() {
        let idx = engine.set_active_bus(name);
        if idx != i as i32 {
            return Err(EngineError::Other(format!(
                "bus capture: SetActiveBus({name:?}) returned {idx}, expected {i} \
                 (AllBusNames must be the engine's BusList order)"
            )));
        }
        let nodes = engine.bus_nodes()?;
        let kv_base = engine.bus_kvbase();
        let pu_voltages = engine.bus_pu_voltages()?;
        let vmag_angle = engine.bus_vmag_angle()?;
        let pu_vmag_angle = engine.bus_pu_vmag_angle()?;
        if nodes.windows(2).any(|w| w[1] <= w[0]) {
            return Err(EngineError::Other(format!(
                "bus capture: {name}.Nodes {nodes:?} is not strictly ascending \
                 (DBus.pas:319-345 walks FindIdx(jj) upward)"
            )));
        }
        for (key, v) in [
            ("pu_voltages", &pu_voltages),
            ("vmag_angle", &vmag_angle),
            ("pu_vmag_angle", &pu_vmag_angle),
        ] {
            if v.len() != 2 * nodes.len() {
                return Err(EngineError::Other(format!(
                    "bus capture: {name}.{key} returned {} values, expected 2*{} for nodes {nodes:?}",
                    v.len(),
                    nodes.len()
                )));
            }
        }
        let (zsc1, zsc0, zsc, ysc, isc, voc) = if want_sc {
            (
                engine.bus_zsc1()?,
                engine.bus_zsc0()?,
                engine.bus_zsc_matrix()?,
                engine.bus_ysc_matrix()?,
                engine.bus_isc()?,
                engine.bus_voc()?,
            )
        } else {
            Default::default()
        };
        if want_sc {
            let n = nodes.len();
            // `Isc`/`Voc` publish `2*n` doubles from the allocated
            // `BusCurrent`/`VBus`, EXCEPT on a 0-node bus, where
            // `Reallocmem(ptr, 0)` frees the pointer and the arm falls back to
            // the sentinel (see `R4133_SC_SENTINEL_LEN`).
            let per_node = if n == 0 { R4133_SC_SENTINEL_LEN } else { 2 * n };
            let matrix = [R4133_SC_SENTINEL_LEN, 2 * n * n];
            for (key, v, want) in [
                ("zsc1", &zsc1, &[2usize][..]),
                ("zsc0", &zsc0, &[2][..]),
                ("zsc", &zsc, &matrix[..]),
                ("ysc", &ysc, &matrix[..]),
                ("isc", &isc, &[per_node][..]),
                ("voc", &voc, &[per_node][..]),
            ] {
                if !want.contains(&v.len()) {
                    return Err(EngineError::Other(format!(
                        "bus capture: {name}.{key} returned {} values, expected one of \
                         {want:?} for nodes {nodes:?} (see BusCap for each arm's Pascal shape)",
                        v.len()
                    )));
                }
            }
        }
        out.push(BusCap {
            name: name.clone(),
            kv_base,
            nodes,
            pu_voltages,
            vmag_angle,
            pu_vmag_angle,
            zsc1,
            zsc0,
            zsc,
            ysc,
            isc,
            voc,
        });
    }
    engine.assert_clean("buses")?;
    Ok(out)
}

/// `Circuit.AllBusMagPu` — every NODE's per-unit voltage magnitude
/// (`CircuitV(9)`, `DCircuit.pas:481-500` == `CAPI_Circuit.pas:521-548`).
///
/// Ordered bus-list order x the bus's INTERNAL node index (`GetRef(j)` for
/// `j = 1..NumNodesThisBus`, i.e. the `AllNodeNames` permutation) — neither the
/// ascending-node-number order of [`capture_all_buses`] nor the gated
/// `YNodeOrder`. Its length is `NumNodes`, which is also the sum of the per-bus
/// node counts: the caller checks that identity, so the two walks cannot drift
/// apart silently.
fn capture_all_bus_vmag_pu(engine: &Engine) -> Result<Vec<f64>, EngineError> {
    let v = engine.circuit_all_bus_mag_pu()?;
    engine.assert_clean("all_bus_vmag_pu")?;
    Ok(v)
}

fn capture_variables(engine: &Engine, names: &[String]) -> Result<Vec<VariablesCap>, EngineError> {
    let mut out = Vec::new();
    for name in names {
        engine.set_active_element(name);
        out.push(VariablesCap {
            name: name.clone(),
            var_names: engine.element_variable_names(),
            values: engine.element_variable_values(),
        });
    }
    if !names.is_empty() {
        engine.assert_clean("variables")?;
    }
    Ok(out)
}

fn capture_ctrlqueue(engine: &Engine) -> Vec<String> {
    engine
        .ctrl_queue()
        .into_iter()
        .filter(|r| {
            let t = r.trim();
            !t.is_empty() && t != "No events" && !r.starts_with("Handle,")
        })
        .collect()
}

/// `capture_eventlog` (Oddie path): `export eventlog` writes a UTF-8-BOM CSV;
/// read it back stripping the BOM per line and dropping blank lines.
fn capture_eventlog(engine: &Engine) -> Result<Vec<String>, EngineError> {
    let reply = engine.raw_command("export eventlog");
    let (errno, desc) = engine.poll_error();
    if errno != 0 {
        return Err(EngineError::Dss {
            errno,
            desc,
            ctx: "export eventlog".to_string(),
        });
    }
    let path = reply.trim().trim_start_matches('\u{FEFF}').to_string();
    let Ok(bytes) = std::fs::read(&path) else {
        return Ok(Vec::new());
    };
    // utf-8-sig: strip a leading file BOM, then per-line BOM + CR.
    let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    let text = String::from_utf8_lossy(body);
    let mut out = Vec::new();
    for raw in text.split('\n') {
        let line = raw
            .trim_start_matches('\u{FEFF}')
            .trim_end_matches(['\r', '\n']);
        if !line.trim().is_empty() {
            out.push(line.to_string());
        }
    }
    Ok(out)
}

/// Read the `<CircuitName>_AutoAddLog.csv` the AutoAdd solve wrote (inside the
/// guard scope, before it removes the file). Newlines are normalized to `\n`:
/// `oracle_server` reads the file in Python text mode (universal newlines, so
/// `\r\n`/`\r` → `\n`), and the capture must match that byte-for-byte.
fn read_autoadd_log(engine: &Engine, case_path: &str) -> Option<String> {
    // Circuit name via CircuitS mode 0 (Name).
    let name = engine.circuit_name();
    let dir = Path::new(case_path).parent()?;
    let log = dir.join(format!("{name}_AutoAddLog.csv"));
    let raw = std::fs::read_to_string(log).ok()?;
    Some(raw.replace("\r\n", "\n").replace('\r', "\n"))
}

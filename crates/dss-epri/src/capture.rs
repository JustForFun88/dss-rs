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

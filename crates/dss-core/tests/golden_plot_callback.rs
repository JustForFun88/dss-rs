//! Plot/Visualize callback goldens (WPG.17): pin the **`plotParams` JSON payload**
//! the pinned oracle hands to `DSS.DSSPlotCallback`. `tools/golden/gen_plot_callback.py`
//! registers a capturing plot callback on the pinned engine, replays a tiny
//! self-contained fixture, issues each plot/visualize variant, and saves the
//! captured JSON to `tests/golden/plot_callback/<variant>.json` (+ a `meta.json`
//! carrying the exact deck + variant commands, so the Rust and oracle fixtures
//! can never drift).
//!
//! This driver replays the same deck + commands on the Rust engine with a
//! capturing `register_plot_callback`, and compares the captured payload to the
//! golden **structurally, with numbers by tolerance** — never a raw float-string
//! diff: fpjson prints `2.0000000000000000E+003`, byte-incomparable with any
//! Rust serializer (PORTING_PLAN §4). Key order is not required (structural);
//! strings/bools/colors compare exact (the color HTML pins the `clXXX` palette).
//!
//! Regenerate only manually: `python tools/golden/gen_plot_callback.py`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use dss_core::exec::Dss;
use serde::Deserialize;
use serde_json::Value;

fn plot_dir() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "plot_callback",
    ]
    .iter()
    .collect()
}

#[derive(Debug, Deserialize)]
struct Variant {
    stem: String,
    cmds: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Meta {
    deck: Vec<String>,
    variants: Vec<Variant>,
}

/// Recursively compare the two payloads: objects by key set + values, arrays
/// elementwise, strings/bools exact, numbers within a tight tolerance (the plot
/// payload carries only exact, parse-derived values — no faer floats).
fn compare_json(oracle: &Value, rust: &Value, path: &str) {
    match (oracle, rust) {
        (Value::Object(o), Value::Object(r)) => {
            let ok: std::collections::BTreeSet<_> = o.keys().collect();
            let rk: std::collections::BTreeSet<_> = r.keys().collect();
            assert_eq!(
                ok, rk,
                "{path}: key set differs (oracle {ok:?} vs rust {rk:?})"
            );
            for (k, ov) in o {
                compare_json(ov, &r[k], &format!("{path}.{k}"));
            }
        }
        (Value::Array(o), Value::Array(r)) => {
            assert_eq!(o.len(), r.len(), "{path}: array length differs");
            for (i, (ov, rv)) in o.iter().zip(r).enumerate() {
                compare_json(ov, rv, &format!("{path}[{i}]"));
            }
        }
        (Value::String(o), Value::String(r)) => {
            assert_eq!(o, r, "{path}: string differs");
        }
        (Value::Bool(o), Value::Bool(r)) => {
            assert_eq!(o, r, "{path}: bool differs");
        }
        (Value::Number(o), Value::Number(r)) => {
            let (a, b) = (o.as_f64().unwrap(), r.as_f64().unwrap());
            let allowed = 1e-9 + 1e-9 * b.abs();
            assert!(
                (a - b).abs() <= allowed,
                "{path}: number differs: oracle {a} vs rust {b}"
            );
        }
        (Value::Null, Value::Null) => {}
        _ => panic!("{path}: type mismatch: oracle {oracle} vs rust {rust}"),
    }
}

#[test]
fn plot_callback_payloads_match_oracle() {
    let dir = plot_dir();
    let meta: Meta = serde_json::from_str(
        &std::fs::read_to_string(dir.join("meta.json")).expect("read plot meta.json"),
    )
    .expect("parse plot meta.json");

    for variant in &meta.variants {
        // `Arc<Mutex>` (test-only sink) so the registered closure is `Send` —
        // the engine is `Send` (P7 rider), so the callback bound is `+ Send`.
        let cap: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let mut dss = Dss::new();
        let sink = cap.clone();
        dss.register_plot_callback(move |json| {
            sink.lock().unwrap().push(json.to_string());
            0
        });

        for c in &meta.deck {
            dss.command(c);
        }
        assert!(
            dss.errors().is_empty(),
            "variant {}: deck errors: {:?}",
            variant.stem,
            dss.errors()
        );
        for c in &variant.cmds {
            dss.command(c);
        }

        let captured = cap.lock().unwrap();
        assert_eq!(
            captured.len(),
            1,
            "variant {}: expected exactly one payload, got {} (errors: {:?})",
            variant.stem,
            captured.len(),
            dss.errors()
        );
        let rust: Value = serde_json::from_str(&captured[0]).expect("rust payload is valid JSON");

        let golden_path = dir.join(format!("{}.json", variant.stem));
        let oracle: Value = serde_json::from_str(
            &std::fs::read_to_string(&golden_path)
                .unwrap_or_else(|e| panic!("read golden {}: {e}", golden_path.display())),
        )
        .expect("parse golden payload");

        compare_json(&oracle, &rust, &variant.stem);
    }
}

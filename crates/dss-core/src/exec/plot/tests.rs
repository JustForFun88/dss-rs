//! Rust-only unit tests for the Plot/Visualize callback surface (WPG.17). These
//! pin the parse → JSON behavior and the callback gating WITHOUT the oracle; the
//! oracle-pinned byte content of the payload is gated by the golden capture
//! (`tests/golden_plot_callback.rs` / `tools/golden/gen_plot_callback.py`).

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::Value;

use crate::exec::Dss;

/// A capturing plot callback backed by a shared `Vec<String>`.
type Cap = Rc<RefCell<Vec<String>>>;

fn dss_with_capture() -> (Dss, Cap) {
    let mut dss = Dss::new();
    let cap: Cap = Rc::new(RefCell::new(Vec::new()));
    let sink = cap.clone();
    dss.register_plot_callback(move |json| {
        sink.borrow_mut().push(json.to_string());
        0
    });
    (dss, cap)
}

fn solved_circuit(dss: &mut Dss) {
    for c in [
        "new circuit.t basekv=12.47 bus1=src",
        "new line.l1 bus1=src bus2=b",
        "new load.ld1 bus1=b kv=12.47 kw=100",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(
        dss.errors().is_empty(),
        "fixture errors: {:?}",
        dss.errors()
    );
}

/// Parse the single captured payload into a JSON `Value`.
fn only_payload(cap: &Cap) -> Value {
    let g = cap.borrow();
    assert_eq!(g.len(), 1, "expected exactly one payload, got {}", g.len());
    serde_json::from_str(&g[0]).expect("payload is valid JSON")
}

#[test]
fn unregistered_plot_is_total_noop() {
    let mut dss = Dss::new(); // no callback registered
    for c in [
        "new circuit.t basekv=12.47 bus1=src",
        "new line.l1 bus1=src bus2=b",
        "solve",
        "plot type=circuit quantity=Power Max=2000",
        "AddBusMarker Bus=b code=5 color=Red size=3",
    ] {
        dss.command(c);
    }
    // Nothing captured (there is no sink), and no error is logged for the
    // now-ported Plot/AddBusMarker commands (previously `AddBusMarker` hit
    // `not_ported_command`).
    assert!(
        dss.errors().is_empty(),
        "unexpected errors: {:?}",
        dss.errors()
    );
}

#[test]
fn register_then_unregister_gates_the_callback() {
    let (mut dss, cap) = dss_with_capture();
    solved_circuit(&mut dss);
    dss.command("plot type=circuit");
    assert_eq!(
        cap.borrow().len(),
        1,
        "registered callback should fire once"
    );

    dss.unregister_plot_callback();
    dss.command("plot type=circuit");
    assert_eq!(
        cap.borrow().len(),
        1,
        "unregistered callback must be silent"
    );

    // Re-registering works.
    let sink = cap.clone();
    dss.register_plot_callback(move |json| {
        sink.borrow_mut().push(json.to_string());
        0
    });
    dss.command("plot type=circuit");
    assert_eq!(cap.borrow().len(), 2, "re-registered callback should fire");
}

#[test]
fn unsolved_guard_suppresses_callback() {
    let (mut dss, cap) = dss_with_capture();
    // Circuit exists but is not solved: `type=circuit` (letter C) trips #24732.
    dss.command("new circuit.t basekv=12.47 bus1=src");
    dss.command("new line.l1 bus1=src bus2=b");
    dss.command("plot type=circuit");
    assert!(
        cap.borrow().is_empty(),
        "unsolved guard must fire, no callback"
    );
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("must be solved before")),
        "expected #24732 message, got {:?}",
        dss.errors()
    );
}

#[test]
fn priceshape_bypasses_the_unsolved_guard() {
    let (mut dss, cap) = dss_with_capture();
    // PriceShape is exempt from the solved-circuit guard (the `pri…` exception),
    // so the callback fires even on an unsolved circuit — and Quantity is forced
    // to 'None' by the post-parse override (unsolved).
    dss.command("new circuit.t basekv=12.47 bus1=src");
    dss.command("plot type=priceshape");
    let v = only_payload(&cap);
    assert_eq!(v["PlotType"], "PriceShape");
    assert_eq!(v["Quantity"], "None");
}

#[test]
fn circuit_payload_fields() {
    let (mut dss, cap) = dss_with_capture();
    solved_circuit(&mut dss);
    dss.command("plot type=circuit quantity=Power Max=2000 subs=y C1=Blue 1phlinestyle=3");
    let v = only_payload(&cap);
    assert_eq!(v["PlotType"], "Circuit");
    assert_eq!(v["Quantity"], "Powers");
    assert_eq!(v["MaxScale"], 2000.0);
    assert_eq!(v["MaxScaleIsSpecified"], true);
    assert_eq!(v["MinScaleIsSpecified"], false);
    assert_eq!(v["ShowSubs"], true);
    assert_eq!(v["SinglePhLineStyle"], 3);
    assert_eq!(v["ThreePhLineStyle"], 1);
    assert_eq!(v["Color1"], "#0000FF"); // clBlue
    assert_eq!(v["Color2"], "#008000"); // clGreen default
    assert_eq!(v["Color3"], "#FF0000"); // clRed default
    assert_eq!(v["TriColorMax"], 0.85);
    assert_eq!(v["TriColorMid"], 0.5);
    assert_eq!(v["Channels"], serde_json::json!([1, 3, 5]));
    assert_eq!(v["Bases"], serde_json::json!([1.0, 1.0, 1.0]));
    assert_eq!(v["DaisySize"], 1.0);
    assert_eq!(v["PhasesToPlot"], -1);
    // The Markers object holds the Circuit.pas defaults.
    assert_eq!(v["Markers"]["NodeMarkerCode"], 16);
    assert_eq!(v["Markers"]["TransMarkerCode"], 35);
    assert_eq!(v["Markers"]["MarkRelays"], false);
    assert_eq!(v["BusMarkers"], serde_json::json!([]));
}

#[test]
fn profile_phases_and_type_variants() {
    let (mut dss, cap) = dss_with_capture();
    solved_circuit(&mut dss);

    dss.command("plot type=profile phases=all");
    assert_eq!(only_payload(&cap)["PhasesToPlot"], -2);
    assert_eq!(only_payload(&cap)["PlotType"], "Profile");
    cap.borrow_mut().clear();

    dss.command("plot type=profile phases=primary");
    assert_eq!(only_payload(&cap)["PhasesToPlot"], -3);
    cap.borrow_mut().clear();

    dss.command("plot type=daisy");
    assert_eq!(only_payload(&cap)["PlotType"], "Daisy");
    cap.borrow_mut().clear();

    // The `type=Losses → LoadShape` quirk (letter L maps unconditionally).
    dss.command("plot type=Losses");
    assert_eq!(only_payload(&cap)["PlotType"], "LoadShape");
    cap.borrow_mut().clear();

    // Monitor + channels/bases vectors.
    dss.command("plot type=monitor object=m1 channels=(1,3,5,7) base=[7200 7200 7200 7200]");
    let v = only_payload(&cap);
    assert_eq!(v["PlotType"], "Monitor");
    assert_eq!(v["ObjectName"], "m1");
    assert_eq!(v["Channels"], serde_json::json!([1, 3, 5, 7]));
    assert_eq!(
        v["Bases"],
        serde_json::json!([7200.0, 7200.0, 7200.0, 7200.0])
    );
}

#[test]
fn add_and_clear_bus_markers() {
    let (mut dss, cap) = dss_with_capture();
    solved_circuit(&mut dss);
    dss.command("AddBusMarker Bus=b code=5 color=Red size=3");
    dss.command("plot type=circuit");
    let v = only_payload(&cap);
    assert_eq!(
        v["BusMarkers"],
        serde_json::json!([{"Name": "b", "Color": "#FF0000", "Code": 5, "Size": 3}])
    );
    cap.borrow_mut().clear();

    dss.command("ClearBusMarkers");
    dss.command("plot type=circuit");
    assert_eq!(only_payload(&cap)["BusMarkers"], serde_json::json!([]));
}

#[test]
fn visualize_payload_and_guards() {
    let (mut dss, cap) = dss_with_capture();
    solved_circuit(&mut dss);
    dss.command("visualize powers Line.l1");
    let v = only_payload(&cap);
    assert_eq!(v["PlotType"], "Visualize");
    assert_eq!(v["ElementName"], "l1");
    assert_eq!(v["ElementType"], "Line");
    assert_eq!(v["Quantity"], "Power");
    cap.borrow_mut().clear();

    // `visualize voltage`/`current` map the quantity by first letter.
    dss.command("visualize current Line.l1");
    assert_eq!(only_payload(&cap)["Quantity"], "Current");
    cap.borrow_mut().clear();
    dss.command("visualize voltage Line.l1");
    assert_eq!(only_payload(&cap)["Quantity"], "Voltage");
    cap.borrow_mut().clear();

    // Element-not-found: #282-equivalent error, no callback.
    dss.command("visualize powers Line.nope");
    assert!(
        cap.borrow().is_empty(),
        "not-found must suppress the callback"
    );
    assert!(
        dss.errors().iter().any(|e| e.contains("not found")),
        "expected not-found error, got {:?}",
        dss.errors()
    );
}

#[test]
fn visualize_unsolved_guard() {
    let (mut dss, cap) = dss_with_capture();
    dss.command("new circuit.t basekv=12.47 bus1=src");
    dss.command("new line.l1 bus1=src bus2=b");
    dss.command("visualize powers Line.l1");
    assert!(cap.borrow().is_empty(), "unsolved visualize must not fire");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("must be solved before")),
        "expected unsolved guard, got {:?}",
        dss.errors()
    );
}

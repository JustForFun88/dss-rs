//! P5b snapshot tests: the fancy (miette graphical) render of engine
//! diagnostics underlines the offending token against the command-line source.
//!
//! The graphical handler is a **test-only** dependency (`dss-core/Cargo.toml`
//! dev-deps enable miette `fancy-no-backtrace`); the shipped library stays
//! protocol-only. Rendered with `GraphicalTheme::unicode_nocolor` at a fixed
//! width so the snapshots are ANSI-free and terminal-independent.

use dss_core::diag::DssDiagnostic;
use dss_core::exec::Dss;
use miette::{GraphicalReportHandler, GraphicalTheme};

/// Render one diagnostic exactly the way `dss-cli --diag=pretty` will.
fn render(d: &DssDiagnostic) -> String {
    let mut out = String::new();
    GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor())
        .with_width(80)
        .render_report(&mut out, d)
        .expect("render");
    out
}

/// The last recorded diagnostic that carries a source span (the one P5b attached).
fn last_with_span(dss: &Dss) -> &DssDiagnostic {
    dss.errors()
        .iter()
        .rev()
        .find(|d| d.src.is_some())
        .expect("a span-carrying diagnostic was recorded")
}

#[test]
fn bad_property_value_underlines_the_value() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.diagprobe");
    dss.command("New Load.x phases=abc");
    let got = render(last_with_span(&dss));
    // A non-numeric integer value is wrapped in `(...)` and parsed as inline
    // RPN (Pascal `GetInteger`), so the message is the RPN entry error; the P5b
    // span underlines the offending value `abc` in the original command line.
    let expected = "  ⚠ Invalid inline math entry: \"abc\"\n   \
        ╭─[<command>:1:19]\n \
        1 │ New Load.x phases=abc \n   \
        ·                   ───\n   \
        ╰────\n";
    assert_eq!(got, expected, "\n--- actual ---\n{got}");
}

#[test]
fn unknown_command_underlines_the_command_name() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.diagprobe");
    dss.command("Frobnicate the widget");
    let got = render(last_with_span(&dss));
    let expected = "  ⚠ Unknown Command: \"Frobnicate\"\n   \
        ╭─[<command>:1:1]\n \
        1 │ Frobnicate the widget \n   \
        · ──────────\n   \
        ╰────\n";
    assert_eq!(got, expected, "\n--- actual ---\n{got}");
}

#[test]
fn redirect_error_origin_carries_file_and_line() {
    let dir = std::env::temp_dir().join(format!("dss_p5b_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let deck = dir.join("broken.dss");
    // The bad property sits on line 3 of the deck; the origin must say ":3".
    std::fs::write(
        &deck,
        "clear\nnew circuit.diagprobe\nNew Load.x phases=abc\n",
    )
    .unwrap();

    let mut dss = Dss::new();
    dss.command(&format!("redirect \"{}\"", deck.display()));

    let d = last_with_span(&dss);
    let got = render(d);
    // The rendered header names the deck file and the deck line number (3) —
    // the P5b scope rule: one command line = one source, origin "<file>:<line>".
    assert!(
        got.contains("broken.dss:3"),
        "origin should be <file>:<line>, got:\n{got}"
    );
    // The offending command line is the source, with `abc` underlined.
    assert!(
        got.contains("New Load.x phases=abc"),
        "source line, got:\n{got}"
    );
    assert!(got.contains("───"), "underline present, got:\n{got}");

    let _ = std::fs::remove_dir_all(&dir);
}

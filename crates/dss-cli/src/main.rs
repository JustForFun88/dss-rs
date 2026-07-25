#![forbid(unsafe_code)]
//! CLI for the DSS engine: compile a `.dss` script and report the solution
//! node voltages (the Phase 3 ★-slice driver; see PORTING_PLAN.md).
//!
//! P5c — diagnostic presentation lives in the binary. The library carries
//! structured [`DssDiagnostic`](dss_core::diag::DssDiagnostic) data (code, span,
//! source); this binary renders it. `--diag=pretty` installs miette's graphical
//! handler and renders each engine diagnostic with source underlines;
//! `--diag=plain` (the default) prints `error: <message>` so drivers, scripts,
//! and the golden harness see byte-identical output unless they opt in.

use std::process::ExitCode;

use dss_core::exec::Dss;

/// How engine diagnostics are rendered to stderr.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DiagMode {
    /// `error: <message>` — one plain line per diagnostic (default).
    Plain,
    /// miette graphical render: code, message, and a source underline.
    Pretty,
}

fn main() -> ExitCode {
    let mut diag = DiagMode::Plain;
    let mut scripts: Vec<String> = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--diag=plain" => diag = DiagMode::Plain,
            "--diag=pretty" => diag = DiagMode::Pretty,
            other if other.starts_with("--diag=") => {
                eprintln!("usage: dss-cli [--diag=pretty|plain] <script.dss> [more scripts...]");
                return ExitCode::FAILURE;
            }
            _ => scripts.push(arg),
        }
    }

    if scripts.is_empty() {
        eprintln!("usage: dss-cli [--diag=pretty|plain] <script.dss> [more scripts...]");
        return ExitCode::FAILURE;
    }

    if diag == DiagMode::Pretty {
        // Install the graphical handler once (binary-only). `Report`'s `Debug`
        // impl renders through it; a failure just means an earlier install won.
        let _ = miette::set_hook(Box::new(|_| {
            Box::new(miette::MietteHandlerOpts::new().build())
        }));
    }

    let mut dss = Dss::new();
    for script in &scripts {
        dss.command(&format!("Compile \"{script}\""));
    }

    let failed = !dss.errors().is_empty();
    for e in dss.errors() {
        match diag {
            DiagMode::Plain => eprintln!("error: {e}"),
            DiagMode::Pretty => eprintln!("{:?}", miette::Report::new(e.clone())),
        }
    }

    if let Some(ckt) = dss.circuit() {
        if ckt.solution_was_attempted {
            println!(
                "circuit \"{}\": {} ({} nodes, {} iterations, max error {:e})",
                ckt.name,
                if ckt.is_solved {
                    "CONVERGED"
                } else {
                    "DID NOT CONVERGE"
                },
                ckt.num_nodes,
                ckt.solution.iteration,
                ckt.solution.max_error,
            );
            println!("{:<24}{:>16}{:>12}", "Node", "|V| (V)", "angle (deg)");
            for i in 1..=ckt.num_nodes {
                let v = ckt.solution.node_v[i];
                println!(
                    "{:<24}{:>16.6}{:>12.4}",
                    ckt.node_name(i),
                    v.norm(),
                    v.arg().to_degrees()
                );
            }
        } else {
            println!(
                "circuit \"{}\": defined, not solved ({} devices)",
                ckt.name, ckt.num_devices
            );
        }
    } else {
        println!("no circuit defined");
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

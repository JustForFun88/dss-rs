#![forbid(unsafe_code)]
//! CLI for the DSS engine: compile a `.dss` script and report the solution
//! node voltages (the Phase 3 ★-slice driver; see PORTING_PLAN.md).

use std::process::ExitCode;

use dss_core::exec::Dss;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(script) = args.next() else {
        eprintln!("usage: dss-cli <script.dss> [more scripts...]");
        return ExitCode::FAILURE;
    };

    let mut dss = Dss::new();
    dss.command(&format!("Compile \"{script}\""));
    for extra in args {
        dss.command(&format!("Compile \"{extra}\""));
    }

    let failed = !dss.errors().is_empty();
    for e in dss.errors() {
        eprintln!("error: {e}");
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

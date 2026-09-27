//! `gate-kind --base <rev> [--head <rev>]`: prints the kind of a diff of the
//! repository around the current directory, one `<kind>\t<path>\t<reason>`
//! line per file and a final `GATE_KIND=<kind>` line, and exits 0.
//!
//! With `--head` it grades the committed range `<base>..<head>`, without it
//! `<base>` against the working tree (`git diff --raw -z -M100% <base>` plus
//! every untracked, not ignored path as added), because a stage gate runs
//! before its Commit stage. Any error (a bad argument, a failing git, an
//! unknown rev, a panic) prints its reason and `GATE_KIND=Code`. The tool never
//! decides by itself what to run (`RETRO_FIXES_PLAN.md` RF-I00-05).

#![forbid(unsafe_code)]

use std::path::PathBuf;

const USAGE: &str = "usage: gate-kind --base <rev> [--head <rev>]";

fn parse_args(args: &[String]) -> Result<(String, Option<String>), String> {
    let mut base = None;
    let mut head = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((flag, value)) if flag.starts_with("--") => (flag, Some(value.to_string())),
            _ => (arg.as_str(), None),
        };
        let slot = match flag {
            "--base" => &mut base,
            "--head" => &mut head,
            _ => return Err(format!("unknown argument {arg:?}; {USAGE}")),
        };
        if slot.is_some() {
            return Err(format!("{flag} given twice; {USAGE}"));
        }
        let value = match inline {
            Some(value) => value,
            None => it
                .next()
                .cloned()
                .ok_or_else(|| format!("{flag} needs a value; {USAGE}"))?,
        };
        if value.is_empty() {
            return Err(format!("{flag} needs a value; {USAGE}"));
        }
        *slot = Some(value);
    }
    let base = base.ok_or_else(|| format!("--base is required; {USAGE}"))?;
    Ok((base, head))
}

fn run() -> Result<gate_kind::DiffReport, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (base, head) = parse_args(&args)?;
    let dir: PathBuf = std::env::current_dir().map_err(|e| format!("no current directory: {e}"))?;
    gate_kind::classify_diff_report(&dir, &base, head.as_deref())
}

fn main() {
    match std::panic::catch_unwind(run) {
        Ok(Ok(report)) => {
            for file in &report.files {
                println!("{file}");
            }
            println!("GATE_KIND={}", report.kind());
        }
        Ok(Err(reason)) => {
            println!("error: {reason}");
            println!("GATE_KIND={}", gate_kind::GateKind::Code);
        }
        Err(_) => {
            println!("error: the classifier panicked");
            println!("GATE_KIND={}", gate_kind::GateKind::Code);
        }
    }
}

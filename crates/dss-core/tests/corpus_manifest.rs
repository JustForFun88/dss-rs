//! Corpus manifest coverage gate (CORPUS_TEST_PLAN.md §2) — the "no silent
//! omissions" guarantee. Oracle-free and always-on (runs in the normal
//! `cargo test --workspace`).
//!
//! Asserts a **bijection** between the `.dss` files under
//! `tests/corpus/electricdss-tst/` and the entries across the manifests in
//! `tests/corpus/manifests/`: every `.dss` is listed in **exactly one**
//! manifest, and every manifested path exists on disk. So the corpus and the
//! manifests can never silently drift — adding or removing a `.dss` fails this
//! test until it is classified.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(default)]
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    path: String,
}

fn corpus_root() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect()
}

fn manifests_dir() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "manifests",
    ]
    .iter()
    .collect()
}

/// Recursively collect every `.dss` file under `dir`, as forward-slashed paths
/// relative to `base`.
fn collect_dss(dir: &Path, base: &Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let p = entry.expect("dir entry").path();
        if p.is_dir() {
            collect_dss(&p, base, out);
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dss")) {
            let rel = p
                .strip_prefix(base)
                .expect("under base")
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);
        }
    }
}

#[test]
fn every_dss_is_accounted_for_exactly_once() {
    let root = corpus_root();
    assert!(
        root.is_dir(),
        "vendored corpus missing: {} (run tools/corpus/vendor.py)",
        root.display()
    );

    // Every manifest entry, mapped path -> manifest file; duplicates collected.
    let mdir = manifests_dir();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&mdir)
        .unwrap_or_else(|e| panic!("read {}: {e}", mdir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        // `ad_sweep.json` (WP-AD.4) is an orthogonal A-Diakoptics disposition
        // OVERLAY on the `solvable_now.json` entry points, not an ownership
        // manifest — its paths are deliberately a subset already owned elsewhere,
        // so it must not participate in the exactly-once ownership bijection. Its
        // own coverage (bijective with solvable_now) is checked by
        // `ad_sweep_covers_solvable_now` in `corpus_live.rs`.
        .filter(|p| p.file_name().is_none_or(|n| n != "ad_sweep.json"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no manifests in {}", mdir.display());

    let mut owner: BTreeMap<String, String> = BTreeMap::new();
    let mut dups: Vec<String> = Vec::new();
    for p in &files {
        let fname = p.file_name().unwrap().to_string_lossy().to_string();
        let text =
            std::fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        let m: Manifest =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()));
        for c in m.cases {
            let norm = c.path.replace('\\', "/");
            if let Some(prev) = owner.insert(norm.clone(), fname.clone()) {
                dups.push(format!("{norm}  (in {prev} and {fname})"));
            }
        }
    }
    assert!(
        dups.is_empty(),
        "{} path(s) listed in more than one manifest:\n  {}",
        dups.len(),
        dups.join("\n  ")
    );

    // On-disk .dss set.
    let mut disk: Vec<String> = Vec::new();
    collect_dss(&root, &root, &mut disk);
    let disk_set: std::collections::BTreeSet<String> = disk.into_iter().collect();
    let manifest_set: std::collections::BTreeSet<String> = owner.keys().cloned().collect();

    // Manifested but absent on disk.
    let ghosts: Vec<&String> = manifest_set.difference(&disk_set).collect();
    assert!(
        ghosts.is_empty(),
        "{} manifested path(s) do not exist under {}:\n  {}",
        ghosts.len(),
        root.display(),
        ghosts
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );

    // On disk but not in any manifest — the silent-omission failure.
    let unaccounted: Vec<&String> = disk_set.difference(&manifest_set).collect();
    assert!(
        unaccounted.is_empty(),
        "{} .dss file(s) are not in any manifest (add them to tests/corpus/manifests/):\n  {}",
        unaccounted.len(),
        unaccounted
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );

    assert_eq!(
        disk_set.len(),
        manifest_set.len(),
        "bijection size mismatch (disk {} vs manifests {})",
        disk_set.len(),
        manifest_set.len()
    );
    eprintln!(
        "corpus manifest coverage: {} .dss accounted for across {} manifests",
        disk_set.len(),
        files.len()
    );
}

/// The population guard behind the G1.3d(i) no-meter sentinel normalization
/// (`harness::oracle_meter_name`): **no vendored deck names an EnergyMeter `0`**,
/// so the r4133 `CktElementS` default (`DDLL/DCktElement.pas:421`) can never
/// collide with a real name in this corpus.
///
/// It lives in this oracle-free hygiene binary, not next to the comparator in
/// `harness/mod.rs` (compiled into 22 test binaries) and not in `corpus_gate`
/// beside the live gate: the scan then runs **once** per `cargo test` AND never
/// concurrently with a deck solve. Measured 2026-09-04 (G1.3d(i) F5): decks
/// write exports into the corpus tree while they solve, and reading one of them
/// mid-write fails with a Windows sharing violation — a probe replicating this
/// walk against the live gate hit
/// `EPRITestCircuits/ckt7/ckt7_Power_elem_kVA.txt` "Permission denied", which is
/// exactly how this test failed once in the corpus-gate binary. `cargo` runs test
/// binaries one at a time, so here there is no concurrent writer at all — a
/// premise, not a law: it holds for `cargo`'s own sequential runner plus the
/// one-gate-per-worktree rule (coordinator decision D13), and a parallel runner
/// (`cargo-nextest`) or two concurrent `cargo test` invocations in one worktree
/// would put a live-gate writer back beside this walk.
///
/// Leftover exports from an earlier run are still counted (the file count is
/// therefore `>=`, not `==`); they can only ADD names, never remove one, so the
/// `0` assertion below stays conservative.
///
/// This census is the **load-bearing** guard for that collision, not a
/// belt-and-braces one: on the r4133 channel a meter named `0` is
/// indistinguishable from the sentinel in BOTH directions — a port that HAS the
/// name reds
/// (`harness::element_extras_pins::a_meter_named_zero_reds_instead_of_passing`),
/// but a port that LOST it would compare `None == None` and pass
/// (`..::the_r4133_zero_sentinel_is_undecidable_and_the_census_is_the_guard`).
/// What makes the case unreachable is this walk (G1.3d(i) audit settlement,
/// 2026-09-05).
mod extras_population {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    /// Extensions a deck can be `Compile`d or `Redirect`ed from.
    const DECK_EXTS: [&str; 4] = ["dss", "dsc", "dsv", "txt"];

    /// Collect every `New EnergyMeter.<name>` spelling in one file's bytes,
    /// lowercased the way both engines and the port store the name (r4133
    /// `Meters/EnergyMeter.pas:921`). Byte-oriented on purpose: some vendored
    /// decks are not valid UTF-8.
    fn meter_names_in(bytes: &[u8], out: &mut BTreeSet<String>) {
        let text = String::from_utf8_lossy(bytes).to_ascii_lowercase();
        let b = text.as_bytes();
        let mut i = 0usize;
        while let Some(off) = text[i..].find("energymeter.") {
            let at = i + off;
            i = at + "energymeter.".len();
            // Backwards over an optional quote and the whitespace to the `new`
            // keyword; anything else (`edit`, `~`, an export line) is not a
            // definition and is skipped.
            let mut j = at;
            if j > 0 && (b[j - 1] == b'"' || b[j - 1] == b'\'') {
                j -= 1;
            }
            while j > 0 && b[j - 1].is_ascii_whitespace() {
                j -= 1;
            }
            if j < 3 || &b[j - 3..j] != b"new" {
                continue;
            }
            if j > 3 {
                let prev = b[j - 4];
                if prev.is_ascii_alphanumeric() || prev == b'_' || prev == b'.' {
                    continue;
                }
            }
            let start = i;
            let mut end = start;
            while end < b.len()
                && !matches!(
                    b[end],
                    b' ' | b'\t' | b'\r' | b'\n' | b',' | b'"' | b'\'' | b'!' | b')'
                )
            {
                end += 1;
            }
            if end > start {
                out.insert(text[start..end].to_string());
            }
            i = end;
        }
    }

    /// `(distinct meter names, deck files scanned)` over the vendored corpus.
    fn census() -> (BTreeSet<String>, usize) {
        let root: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "tests", "corpus"]
            .iter()
            .collect();
        let mut names = BTreeSet::new();
        let mut files = 0usize;
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            let entries = std::fs::read_dir(&dir)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));
            for entry in entries {
                let path = entry.expect("corpus dir entry").path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let is_deck = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| DECK_EXTS.contains(&e.to_ascii_lowercase().as_str()));
                if !is_deck {
                    continue;
                }
                files += 1;
                let bytes = std::fs::read(&path)
                    .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
                meter_names_in(&bytes, &mut names);
            }
        }
        (names, files)
    }

    /// Measured 2026-09-04 on the vendored corpus: **1310** deck files, **92**
    /// distinct meter names, none of them `0`. The bounds are deliberately loose
    /// (another lane may add a deck) but non-vacuous: a scan that broke — a wrong
    /// root, a parser that matches nothing — fails here instead of greening.
    #[test]
    fn no_corpus_energymeter_is_named_zero() {
        let (names, files) = census();
        assert!(
            files >= 1_000 && names.len() >= 80,
            "the EnergyMeter census looks broken: {files} deck file(s), \
             {} name(s) (measured 1310 / 92 on 2026-09-04)",
            names.len()
        );
        for want in ["feeder", "em", "m1", "25607", "k21_feeder"] {
            assert!(
                names.contains(want),
                "the census lost the known meter name {want:?} — the parser or \
                 the corpus root moved"
            );
        }
        assert!(
            !names.contains("0"),
            "a vendored deck names an EnergyMeter `0`, which collides with the \
             r4133 `CktElementS` no-meter sentinel (DDLL/DCktElement.pas:421) \
             that `harness::oracle_meter_name` normalizes away"
        );
    }

    /// The parser's own rejection legs, so the guard above cannot pass by
    /// matching nothing: a definition is picked up in every spelling the decks
    /// use, and a non-definition mention is not.
    #[test]
    fn the_census_parser_reads_definitions_only() {
        let mut got = BTreeSet::new();
        meter_names_in(
            b"New EnergyMeter.Feeder element=Line.l1 terminal=1\r\n\
              new energymeter.m1 element=line.a\n\
              New \"EnergyMeter.quoted\" element=line.b\n\
              ~ energymeter.later action=save\n\
              Edit EnergyMeter.Feeder localonly=yes\n\
              Export Meters energymeter.feeder\n\
              // renew energymeter.commented\n",
            &mut got,
        );
        assert_eq!(
            got,
            ["feeder", "m1", "quoted"]
                .iter()
                .map(|s| (*s).to_string())
                .collect::<BTreeSet<_>>()
        );
        // A meter named `0` IS found — the guard is not blind to the one name it
        // exists to look for.
        let mut zero = BTreeSet::new();
        meter_names_in(b"new energymeter.0 element=line.a", &mut zero);
        assert!(zero.contains("0"));
        // Invalid UTF-8 does not stop the scan.
        let mut lossy = BTreeSet::new();
        meter_names_in(b"\xff\xfe new energymeter.em2 element=line.a", &mut lossy);
        assert!(lossy.contains("em2"));
    }

    /// The census root is the vendored corpus the gate actually runs, not
    /// `.inputs/` (which is gitignored and absent on a fresh clone).
    #[test]
    fn the_census_root_is_the_vendored_corpus() {
        let root: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "tests", "corpus"]
            .iter()
            .collect();
        assert!(
            Path::new(&root).join("electricdss-tst").is_dir(),
            "the vendored corpus is missing at {}",
            root.display()
        );
    }
}

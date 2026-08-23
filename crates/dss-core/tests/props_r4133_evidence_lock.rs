//! Integrity lock over the vendored r4133 property census
//! (`tests/corpus/props_r4133/`, `R4133_PROPS_PLAN.md` sub-step RP0.1).
//!
//! # Why
//!
//! That directory is frozen evidence: five byte-identical copies of extracts
//! whose source is local-only and gitignored, plus derivations of a 270 MiB
//! census that is not in the repo. Every row of every table `R4133_PROPS_PLAN`
//! introduces has to trace back to a row here, and from RP2.1 on
//! `examples_full.txt` is the *input* of the replay-accounting test — a lost or
//! quietly edited row would not fail that test, it would only shrink what it
//! proves. Nothing else guards these bytes: the tree is deliberately outside
//! `golden_lock.rs`'s scope (recorded there in `EXCLUDED_TREES`, reasoned in the
//! directory's `README.md`), so this file is the guard.
//!
//! # What is locked
//!
//! 1. **Bytes** — SHA-256 + length over the five verbatim copies. They are
//!    hashed **raw**: `.gitattributes` marks the directory `-text` (asserted
//!    below), so the committed bytes and the working-tree bytes are the same
//!    on every platform, CRLF files and the one LF file (`triage.md`) alike.
//! 2. **Row counts** of the derived files (the acceptance criterion of WP-RP0).
//! 3. **Cross-file equalities** — `bins.tsv` ↔ the pair files ↔
//!    `examples_full.txt` ↔ the in-scope pair files ↔ `shape_in_scope.txt`.
//!    These catch a partial regeneration or a hand edit that byte-locks alone
//!    would let through on a derived file.
//! 4. **The two counted claims the README's data traps make** — the 17
//!    heterogeneous structural pairs and bin 1's nine echo-carrying pairs (four
//!    mixed, five pure echo). Both are re-derived here from
//!    `examples_full.txt` + `bins.tsv`, so the corrected numbers are machine-
//!    checked rather than prose.
//! 5. **The RP2.1 supplement** — `examples_supplement.txt`, the measured
//!    spelling inventory of the 26 pairs no frozen row can carry (24 created by
//!    the WP-RP1 shape closures, plus the pair the 2026-08-08 walk missed and
//!    the pair a `SKIP_PROPS` row hid). Row/pair/cell counts, a unique
//!    `(pair, rust, r4133)` triple, disjointness from `bins.tsv`, and the
//!    presence of the provenance header that IS those pairs' bin assignment.
//!
//! Nothing here needs an oracle, a solve or a feature flag: it is a data lock,
//! green in both lanes.
//!
//! A deliberate re-measurement (the RP0.2 knob) that legitimately moves these
//! files updates the constants below in the same commit — that diff is the
//! review artifact.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use sha2::{Digest, Sha256};

/// The vendored evidence directory, repo-root-relative.
const DIR: &str = "tests/corpus/props_r4133";

/// The one file in [`DIR`] that carries `#` comment lines — see [`data_rows`].
const SUPPLEMENT: &str = "examples_supplement.txt";

/// The five verbatim copies: name, SHA-256 over the raw bytes, byte length.
/// Their total is the 24 944 bytes the plan's RP0.1 text and the README quote.
const VERBATIM: &[(&str, &str, usize)] = &[
    (
        "numeric_pairs.txt",
        "ebe8ab5aad20d96e76bb1e52eb12825783aa93fd63f2ef193ff6ef04e01a6a22",
        6088,
    ),
    (
        "shape.txt",
        "1058d24c4606f6009c7357cc70879dedb449ff05fb08f174918bc355313e705e",
        414,
    ),
    (
        "structural_pairs.txt",
        "7834ef48307d75b55f8562ddd00cb9a8891b0f7330630fb8d54cde68bebd268b",
        9395,
    ),
    (
        "summary.json",
        "496e45d801ae9a470d09b5bb56c22614001b6b72a5cd9d4f5710836ed196c4d3",
        81,
    ),
    (
        "triage.md",
        "ec261b373376b035126788afbdaae02e0c961bd64c1135825a1a76cc16256007",
        8966,
    ),
];

/// Data-row counts (headers excluded) of every row-shaped file.
const ROW_COUNTS: &[(&str, usize)] = &[
    ("structural_pairs.txt", 209),
    ("numeric_pairs.txt", 94),
    ("structural_pairs_in_scope.txt", 198),
    ("numeric_pairs_in_scope.txt", 53),
    ("examples_full.txt", 3378),
    ("examples_supplement.txt", 76),
    ("bins.tsv", 303),
    ("shape.txt", 5),
    ("shape_in_scope.txt", 5),
];

/// `examples_supplement.txt` (RP2.1 part C): the pairs no frozen row can carry —
/// 24 the WP-RP1 shape closures made live, plus the one the 2026-08-08 walk
/// missed (`regcontrol.fwdthreshold`, RP0.2 correction 1) and the one a
/// `SKIP_PROPS` row hid on both channels until RP2.1 part A disposed of it
/// (`regcontrol.revthreshold`).
const SUPPLEMENT_PAIRS: usize = 26;
/// Cells behind those 76 rows (the file's own `count` column).
const SUPPLEMENT_CELLS: usize = 4541;

/// `R4133_PROPS_PLAN.md` §1.1: bin → (pairs, cells) over the full census.
const BIN_TOTALS: &[(u8, usize, usize)] = &[
    (1, 75, 297_593),
    (2, 61, 93_213),
    (3, 8, 4_400),
    (4, 21, 122_554),
    (5, 44, 442_369),
    (6, 61, 47_885),
    (7, 33, 47_432),
];

/// Structural bins in scope (`cells_in_scope > 0`), §1.1's 198.
const STRUCTURAL_IN_SCOPE: &[(u8, usize)] = &[(1, 75), (2, 59), (3, 7), (4, 14), (5, 43)];

/// The eleven Delphi boolean spellings r4133 answers with (README §bin 1).
/// Anything else on a bin-1 pair is a `PropertyValue[]` echo.
const BOOL_SPELLINGS: &[&str] = &[
    "true", "True", "false", "False", "YES", "yes", "no", "NO", "n", "y", "Y",
];

/// Bin-1 pairs whose r4133 side is an echo in some or all cells: pair, foldable
/// cells, echo cells, the echo spelling. Four mixed + five pure echo — the
/// measurement that corrects the plan's "three mixed pairs" (README §"Bin 1
/// carries nine echo pairs, not three").
const BIN1_ECHO: &[(&str, usize, usize, &str)] = &[
    ("capcontrol.reset", 0, 446, ""),
    ("recloser.debugtrace", 0, 230, ""),
    ("recloser.eventlog", 30, 200, ""),
    ("regcontrol.idle", 1, 887, ""),
    ("regcontrol.idleforward", 0, 888, ""),
    ("regcontrol.idlereverse", 0, 888, ""),
    ("relay.distreverse", 32, 238, ""),
    ("relay.reset", 226, 44, "0.20"),
    ("upfccontrol.enabled", 0, 13, ""),
];

/// Structural pairs whose cells do **not** all classify into the pair's own
/// bin: pair, its `bins.tsv` bin, its off-bin cell count (full census).
/// `isource.bus1` is excluded — it is the one name that is both a structural
/// and a numeric pair, so `examples_full.txt` cannot separate its two
/// populations (README §"One pair name is both structural and numeric").
const HETEROGENEOUS: &[(&str, u8, usize)] = &[
    ("capcontrol.type", 2, 30),
    ("fault.bus2", 2, 1),
    ("fuse.switchedobj", 2, 22),
    ("invcontrol.mode", 2, 64),
    ("invcontrol.monvoltagecalc", 5, 15),
    ("invcontrol.voltage_curvex_ref", 2, 3),
    ("isource.scantype", 3, 1),
    ("line.wires", 5, 3),
    ("load.yearly", 5, 5_995),
    ("load.zipv", 5, 342),
    ("reactor.bus2", 5, 41),
    ("recloser.switchedobj", 2, 41),
    ("relay.switchedobj", 2, 103),
    ("storage.dynadata", 5, 2),
    ("storagecontroller.seasontargets", 5, 25),
    ("storagecontroller.seasontargetslow", 5, 25),
    ("swtcontrol.action", 3, 6),
];

/// `shape_in_scope.txt`: class → (rows, rows in scope). 429 → 149 (§1.1,
/// WP-RP1's acceptance number).
const SHAPE_IN_SCOPE: &[(&str, usize, usize)] = &[
    ("autotrans", 42, 7),
    ("generator", 273, 137),
    ("sensor", 61, 0),
    ("gendispatcher", 48, 0),
    ("windgen", 5, 5),
];

/// The `shape.txt` classes RP1.1 closes, with the census's own `oracle_count`
/// (the r4133 table's name-list length) and the names the port was missing.
/// `shape.txt` is frozen evidence, so its `rust_count` stays at the pre-RP1.1
/// number — what must be true *now* is that the live table has grown to
/// `oracle_count` and carries the once-missing names.
const RP1_1_CLOSED: &[(&str, usize, &[&str])] = &[
    ("generator", 50, &["rneut", "xneut"]),
    ("sensor", 16, &["action"]),
];

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

fn read(name: &str) -> String {
    let path = repo_root().join(DIR).join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Non-empty lines with the CR of the CRLF files stripped, header dropped when
/// `header` is `Some` (and asserted to be exactly that string).
///
/// `#`-prefixed lines are dropped **for `examples_supplement.txt` only**, whose
/// provenance header IS the bin assignment of the pairs it holds (they have no
/// `bins.tsv` row). The frozen extracts carry no comment line and are not
/// allowed to grow one: for them a `#` line stays a data row, so it fails the
/// row-count lock loudly. (RP2.1 first widened the filter to every file, which
/// quietly made a `#` line inserted into a frozen extract invisible to
/// [`derived_extracts_keep_their_row_counts`] — the only assertion that would
/// have seen it. Audit round.)
fn data_rows(name: &str, header: Option<&str>) -> Vec<String> {
    let comments_allowed = name == SUPPLEMENT;
    let text = read(name);
    let mut rows: Vec<String> = text
        .lines()
        .map(|l| l.trim_end_matches('\r').to_string())
        .filter(|l| !l.is_empty() && !(comments_allowed && l.starts_with('#')))
        .collect();
    if let Some(want) = header {
        let got = rows.first().cloned().unwrap_or_default();
        assert_eq!(got, want, "{name}: unexpected header line");
        rows.remove(0);
    }
    rows
}

/// Parse one `class.prop | 'left' | 'right' | tail` row with the quote anchors
/// the README prescribes — values may contain `|`, never `'`.
fn parse_row(name: &str, line: &str) -> (String, String, String, String) {
    fn bad(name: &str, line: &str) -> ! {
        panic!("{name}: unparsable row {line:?}")
    }
    let (pair, rest) = line.split_once(" | ").unwrap_or_else(|| bad(name, line));
    let rest = rest.strip_prefix('\'').unwrap_or_else(|| bad(name, line));
    let (left, rest) = rest.split_once('\'').unwrap_or_else(|| bad(name, line));
    let rest = rest.strip_prefix(" | '").unwrap_or_else(|| bad(name, line));
    let (right, rest) = rest.split_once('\'').unwrap_or_else(|| bad(name, line));
    let tail = rest.strip_prefix(" | ").unwrap_or_else(|| bad(name, line));
    assert!(
        !pair.contains(' ') && pair.contains('.'),
        "{name}: {pair:?} is not a class.prop pair"
    );
    (
        pair.to_string(),
        left.to_string(),
        right.to_string(),
        tail.to_string(),
    )
}

/// The last `|`-separated field of a pair-file row is its cell count.
fn rows_field(name: &str, tail: &str) -> usize {
    tail.rsplit(" | ")
        .next()
        .unwrap_or(tail)
        .trim()
        .parse()
        .unwrap_or_else(|e| panic!("{name}: bad row count in {tail:?}: {e}"))
}

/// One `bins.tsv` row.
struct Bin {
    pair: String,
    kind: String,
    bin: u8,
    subbin: String,
    cells: usize,
    cells_in_scope: usize,
    max_rel_in_scope: String,
}

fn bins() -> Vec<Bin> {
    let rows = data_rows(
        "bins.tsv",
        Some("pair\tkind\tbin\tsubbin\tcells\tcells_in_scope\tmax_rel\tmax_rel_in_scope"),
    );
    rows.iter()
        .map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            assert_eq!(f.len(), 8, "bins.tsv: {line:?} has {} fields", f.len());
            assert!(
                f[1] == "structural" || f[1] == "numeric",
                "bins.tsv: unknown kind {:?}",
                f[1]
            );
            Bin {
                pair: f[0].to_string(),
                kind: f[1].to_string(),
                bin: f[2].parse().expect("bin"),
                subbin: f[3].to_string(),
                cells: f[4].parse().expect("cells"),
                cells_in_scope: f[5].parse().expect("cells_in_scope"),
                max_rel_in_scope: f[7].to_string(),
            }
        })
        .collect()
}

/// The README's structural classification chain, first match wins. Applied to a
/// **cell** (a distinct `(rust, r4133)` spelling), which is what makes the
/// heterogeneity check below possible.
fn classify(rust: &str, r4133: &str) -> u8 {
    if rust == "Yes" || rust == "No" {
        1
    } else if rust.is_empty() || r4133.is_empty() {
        5
    } else if rust.to_lowercase() == r4133.to_lowercase()
        || rust.trim().to_lowercase() == r4133.trim().to_lowercase()
    {
        2
    } else if rust.starts_with(['[', '(']) || r4133.starts_with(['[', '(']) {
        4
    } else {
        3
    }
}

#[test]
fn verbatim_copies_keep_their_bytes() {
    let mut total = 0usize;
    for (name, want, len) in VERBATIM {
        let path = repo_root().join(DIR).join(name);
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        assert_eq!(
            bytes.len(),
            *len,
            "{name}: {} bytes, expected {len} — this is a verbatim copy of a local-only source; \
             it is never edited in place",
            bytes.len()
        );
        let got = format!("{:x}", Sha256::digest(&bytes));
        assert_eq!(
            got, *want,
            "{name}: content digest moved. The five copies in {DIR} are byte-identical to the \
             G1.1 extracts in the (gitignored) investigations/g1_1_r4133_props/; if a \
             re-measurement legitimately replaced them, update VERBATIM in the same commit."
        );
        total += bytes.len();
    }
    assert_eq!(total, 24_944, "the five copies total 24 944 bytes");
}

/// The byte lock above is only as good as the `-text` attribute that keeps git
/// from EOL-rewriting these files on checkout (`core.autocrlf` is on here).
#[test]
fn the_directory_is_declared_binary_safe() {
    let path = repo_root().join(".gitattributes");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read .gitattributes: {e}"));
    let stanza = format!("{DIR}/** -text");
    assert!(
        text.lines().any(|l| l.trim() == stanza),
        ".gitattributes lost {stanza:?} — without it a checkout under core.autocrlf=true rewrites \
         the verbatim copies' endings and the digests above stop describing the source"
    );
}

#[test]
fn derived_extracts_keep_their_row_counts() {
    for (name, want) in ROW_COUNTS {
        let header = match *name {
            "shape.txt" | "shape_in_scope.txt" => None,
            "bins.tsv" => {
                Some("pair\tkind\tbin\tsubbin\tcells\tcells_in_scope\tmax_rel\tmax_rel_in_scope")
            }
            "examples_full.txt" | "examples_supplement.txt" => {
                Some("class.prop | rust | r4133 | count")
            }
            "numeric_pairs.txt" | "numeric_pairs_in_scope.txt" => {
                Some("class.prop | rust-example | r4133-example | max_rel | rows")
            }
            _ => Some("class.prop | rust-example | r4133-example | rows"),
        };
        let rows = data_rows(name, header);
        assert_eq!(
            rows.len(),
            *want,
            "{name}: {} data rows, expected {want}",
            rows.len()
        );
        // The comment convention is the supplement's alone (see `data_rows`):
        // a `#` line anywhere else is either a hand edit of frozen evidence or
        // a generator change, and either way the lock must see it.
        if *name != SUPPLEMENT {
            assert!(
                !read(name).lines().any(|l| l.starts_with('#')),
                "{name}: a `#`-prefixed line appeared in a frozen extract — only \
                 {SUPPLEMENT} carries comments, and this file's row count is what \
                 guards its bytes"
            );
        }
    }
}

#[test]
fn bins_tsv_agrees_with_the_pair_files() {
    let bins = bins();
    let mut by_key: BTreeMap<(String, String), &Bin> = BTreeMap::new();
    for b in &bins {
        assert!(
            by_key.insert((b.pair.clone(), b.kind.clone()), b).is_none(),
            "bins.tsv: duplicate row for {} / {}",
            b.pair,
            b.kind
        );
    }

    for (file, kind, rows_idx, in_scope) in [
        ("structural_pairs.txt", "structural", 0usize, false),
        ("numeric_pairs.txt", "numeric", 1, false),
        ("structural_pairs_in_scope.txt", "structural", 0, true),
        ("numeric_pairs_in_scope.txt", "numeric", 1, true),
    ] {
        let header = if rows_idx == 0 {
            "class.prop | rust-example | r4133-example | rows"
        } else {
            "class.prop | rust-example | r4133-example | max_rel | rows"
        };
        let mut seen = BTreeSet::new();
        for line in data_rows(file, Some(header)) {
            let (pair, _, _, tail) = parse_row(file, &line);
            let cells = rows_field(file, &tail);
            let b = by_key
                .get(&(pair.clone(), kind.to_string()))
                .unwrap_or_else(|| panic!("{file}: {pair} has no {kind} row in bins.tsv"));
            let want = if in_scope { b.cells_in_scope } else { b.cells };
            assert_eq!(
                cells, want,
                "{file}: {pair} carries {cells} rows, bins.tsv says {want}"
            );
            assert!(seen.insert(pair.clone()), "{file}: {pair} appears twice");
        }
        // Both directions: every bins.tsv row of this kind must be present
        // (in-scope files: exactly the pairs with a kept cell).
        for b in bins.iter().filter(|b| b.kind == kind) {
            let expected = !in_scope || b.cells_in_scope > 0;
            assert_eq!(
                seen.contains(&b.pair),
                expected,
                "{file}: {} {} — bins.tsv says cells_in_scope = {}",
                b.pair,
                if expected {
                    "is missing"
                } else {
                    "should not be here"
                },
                b.cells_in_scope
            );
        }
    }
}

#[test]
fn examples_full_carries_every_pair_and_every_cell() {
    let bins = bins();
    let mut want: BTreeMap<String, usize> = BTreeMap::new();
    for b in &bins {
        *want.entry(b.pair.clone()).or_default() += b.cells;
    }

    let mut got: BTreeMap<String, usize> = BTreeMap::new();
    let mut triples = BTreeSet::new();
    let mut total = 0usize;
    for line in data_rows(
        "examples_full.txt",
        Some("class.prop | rust | r4133 | count"),
    ) {
        let (pair, rust, r4133, tail) = parse_row("examples_full.txt", &line);
        let count = rows_field("examples_full.txt", &tail);
        assert!(
            count > 0,
            "examples_full.txt: {pair} has a zero-count spelling"
        );
        assert!(
            !rust.contains('\t') && !r4133.contains('\t'),
            "examples_full.txt: {pair} value contains a TAB — the parse invariant is broken"
        );
        assert!(
            triples.insert((pair.clone(), rust, r4133)),
            "examples_full.txt: duplicate (pair, rust, r4133) triple in {line:?} — the join key \
             the README prescribes must stay unique"
        );
        *got.entry(pair).or_default() += count;
        total += count;
    }

    assert_eq!(
        total, 1_055_446,
        "examples_full.txt must account for every value cell of the census \
         (960 129 structural + 95 317 numeric)"
    );
    assert_eq!(
        got, want,
        "examples_full.txt's per-pair cell sums must equal bins.tsv's cells column"
    );
}

/// **The RP2.1 supplement.** `examples_supplement.txt` is not a copy of anything
/// — it is a measurement (a full `DSS_PROPS_CENSUS=1` run on the post-RP1.4 tree,
/// 2026-08-23) of the pairs the frozen extracts *structurally cannot* contain,
/// and the RP2.1 replay reads it as one population with `examples_full.txt`.
/// What must hold for that join to be sound: the same row shape, a unique
/// `(pair, rust, r4133)` triple, and **no pair in both files** — a pair on both
/// sides would make the triple join ambiguous and would mean one of the two
/// files is describing a population it should not.
#[test]
fn the_supplement_carries_only_pairs_no_frozen_row_can() {
    let frozen: BTreeSet<String> = bins().into_iter().map(|b| b.pair).collect();
    let mut pairs: BTreeMap<String, usize> = BTreeMap::new();
    let mut triples = BTreeSet::new();
    let mut cells = 0usize;
    for line in data_rows(
        "examples_supplement.txt",
        Some("class.prop | rust | r4133 | count"),
    ) {
        let (pair, rust, r4133, tail) = parse_row("examples_supplement.txt", &line);
        let count = rows_field("examples_supplement.txt", &tail);
        assert!(count > 0, "examples_supplement.txt: {pair} zero-count row");
        assert!(
            !rust.contains('\t') && !r4133.contains('\t'),
            "examples_supplement.txt: {pair} value contains a TAB"
        );
        assert!(
            !frozen.contains(&pair),
            "examples_supplement.txt: {pair} also has a bins.tsv row — the supplement is only \
             for pairs the frozen census could not record"
        );
        assert!(
            triples.insert((pair.clone(), rust, r4133)),
            "examples_supplement.txt: duplicate (pair, rust, r4133) triple in {line:?}"
        );
        *pairs.entry(pair).or_default() += count;
        cells += count;
    }
    assert_eq!(pairs.len(), SUPPLEMENT_PAIRS, "supplement pairs");
    assert_eq!(cells, SUPPLEMENT_CELLS, "supplement cells");

    // The provenance header is the file's bin assignment (these pairs have no
    // `bins.tsv` row), so it is part of the evidence, not decoration.
    let text = read("examples_supplement.txt");
    for marker in [
        "DSS_PROPS_CENSUS=1",
        "regcontrol.fwdthreshold",
        "regcontrol.revthreshold",
        "Pairs the WP-RP1 shape closures make live",
    ] {
        assert!(
            text.contains(marker),
            "examples_supplement.txt lost its provenance header ({marker:?})"
        );
    }
}

#[test]
fn per_bin_totals_match_the_plan() {
    let bins = bins();
    let mut pairs: BTreeMap<u8, usize> = BTreeMap::new();
    let mut cells: BTreeMap<u8, usize> = BTreeMap::new();
    let mut structural_in_scope: BTreeMap<u8, usize> = BTreeMap::new();
    let mut subbin: BTreeMap<String, usize> = BTreeMap::new();
    let (mut display, mut jump) = (0usize, 0usize);

    for b in &bins {
        *pairs.entry(b.bin).or_default() += 1;
        *cells.entry(b.bin).or_default() += b.cells;
        match b.kind.as_str() {
            "structural" => {
                assert!(
                    (1..=5).contains(&b.bin),
                    "{}: structural bin {}",
                    b.pair,
                    b.bin
                );
                if b.cells_in_scope > 0 {
                    *structural_in_scope.entry(b.bin).or_default() += 1;
                }
                *subbin.entry(b.subbin.clone()).or_default() += 1;
            }
            _ => {
                assert!(
                    b.bin == 6 || b.bin == 7,
                    "{}: numeric bin {}",
                    b.pair,
                    b.bin
                );
                assert_eq!(b.subbin, "-", "{}: numeric rows carry no subbin", b.pair);
                if b.cells_in_scope > 0 {
                    // The in-scope numeric split is *re-derived* from
                    // `max_rel_in_scope`, not read off the full-census bin
                    // (README, last data trap): four bin-7 pairs are display
                    // class once the capi-only cases are dropped.
                    let max_rel: f64 = b
                        .max_rel_in_scope
                        .parse()
                        .unwrap_or_else(|e| panic!("{}: max_rel_in_scope: {e}", b.pair));
                    if max_rel >= 1e-4 {
                        jump += 1;
                    } else {
                        display += 1;
                    }
                }
            }
        }
    }

    for (bin, want_pairs, want_cells) in BIN_TOTALS {
        assert_eq!(
            pairs.get(bin).copied().unwrap_or(0),
            *want_pairs,
            "bin {bin} pairs"
        );
        assert_eq!(
            cells.get(bin).copied().unwrap_or(0),
            *want_cells,
            "bin {bin} cells"
        );
    }
    assert_eq!(bins.len(), 303, "303 census pairs");
    assert_eq!(cells.values().sum::<usize>(), 1_055_446, "total cells");
    assert_eq!(
        subbin.get("case").copied().unwrap_or(0),
        59,
        "bin-2 case pairs"
    );
    assert_eq!(
        subbin.get("trail").copied().unwrap_or(0),
        2,
        "bin-2 trailing-space pairs"
    );

    for (bin, want) in STRUCTURAL_IN_SCOPE {
        assert_eq!(
            structural_in_scope.get(bin).copied().unwrap_or(0),
            *want,
            "bin {bin} in-scope pairs"
        );
    }
    assert_eq!(
        structural_in_scope.values().sum::<usize>(),
        198,
        "in-scope structural pairs"
    );
    assert_eq!(
        (display, jump),
        (37, 16),
        "in-scope numeric split (README §per-bin totals)"
    );
}

/// The README's two counted data traps, re-derived from the vendored files.
///
/// Both correct a claim the plan makes, so they are locked rather than left as
/// prose: `bins.tsv`'s one-bin-per-pair label hides 17 heterogeneous pairs, and
/// bin 1 holds nine echo-carrying pairs where `R4133_PROPS_PLAN.md` §1.2 names
/// three.
#[test]
fn the_readme_data_traps_are_still_the_measurement() {
    let bins = bins();
    let bin_of: BTreeMap<(String, String), u8> = bins
        .iter()
        .map(|b| ((b.pair.clone(), b.kind.clone()), b.bin))
        .collect();
    let dual_kind: BTreeSet<String> = bins
        .iter()
        .filter(|b| {
            b.kind == "numeric" && bin_of.contains_key(&(b.pair.clone(), "structural".into()))
        })
        .map(|b| b.pair.clone())
        .collect();
    assert_eq!(
        dual_kind.iter().map(String::as_str).collect::<Vec<_>>(),
        ["isource.bus1"],
        "the README names isource.bus1 as the one pair living in both kinds"
    );

    let mut per_pair_bins: BTreeMap<String, BTreeMap<u8, usize>> = BTreeMap::new();
    let mut bin1: BTreeMap<String, (usize, usize, BTreeSet<String>)> = BTreeMap::new();
    for line in data_rows(
        "examples_full.txt",
        Some("class.prop | rust | r4133 | count"),
    ) {
        let (pair, rust, r4133, tail) = parse_row("examples_full.txt", &line);
        let count = rows_field("examples_full.txt", &tail);
        *per_pair_bins
            .entry(pair.clone())
            .or_default()
            .entry(classify(&rust, &r4133))
            .or_default() += count;
        if bin_of.get(&(pair.clone(), "structural".to_string())) == Some(&1) {
            let e = bin1.entry(pair).or_default();
            if BOOL_SPELLINGS.contains(&r4133.as_str()) {
                e.0 += count;
            } else {
                e.1 += count;
                e.2.insert(r4133);
            }
        }
    }

    // (a) heterogeneity: pairs whose cells do not all land in their own bin.
    let mut het: Vec<(String, u8, usize)> = Vec::new();
    let mut off_bin_total = 0usize;
    for (pair, dist) in &per_pair_bins {
        if dual_kind.contains(pair) {
            continue;
        }
        let Some(&bin) = bin_of.get(&(pair.clone(), "structural".to_string())) else {
            continue;
        };
        let off: usize = dist
            .iter()
            .filter(|(b, _)| **b != bin)
            .map(|(_, c)| *c)
            .sum();
        if off > 0 {
            het.push((pair.clone(), bin, off));
            off_bin_total += off;
        }
    }
    let want: Vec<(String, u8, usize)> = HETEROGENEOUS
        .iter()
        .map(|(p, b, o)| ((*p).to_string(), *b, *o))
        .collect();
    assert_eq!(
        het, want,
        "the set of structural pairs whose cells split across bins moved — the README's \
         \"A pair's bin is a label\" table and RP2.1's admissibility rule are measured on it"
    );
    assert_eq!(off_bin_total, 6_719, "off-bin structural cells");

    // (b) bin 1: nine echo-carrying pairs, four of them mixed.
    let echo: Vec<(String, usize, usize, String)> = bin1
        .iter()
        .filter(|(_, (_, e, _))| *e > 0)
        .map(|(p, (f, e, spellings))| {
            assert_eq!(
                spellings.len(),
                1,
                "{p}: expected one echo spelling, got {spellings:?}"
            );
            (
                p.clone(),
                *f,
                *e,
                spellings.iter().next().cloned().unwrap_or_default(),
            )
        })
        .collect();
    let want: Vec<(String, usize, usize, String)> = BIN1_ECHO
        .iter()
        .map(|(p, f, e, s)| ((*p).to_string(), *f, *e, (*s).to_string()))
        .collect();
    assert_eq!(
        echo, want,
        "bin 1's echo population moved — RP2.3's echo rows are sized on exactly these nine pairs \
         (the plan's \"three mixed pairs\" is the number this measurement corrects)"
    );
    assert_eq!(
        echo.iter().filter(|(_, f, _, _)| *f > 0).count(),
        4,
        "four bin-1 pairs mix foldable booleans with echoes"
    );
    assert_eq!(
        echo.iter().map(|(_, _, e, _)| *e).sum::<usize>(),
        3_834,
        "bin-1 echo cells"
    );
    assert_eq!(bin1.len(), 75, "bin 1 has 75 pairs");
}

#[test]
fn shape_in_scope_splits_shape_txt() {
    let full = data_rows("shape.txt", None);
    let in_scope = data_rows("shape_in_scope.txt", None);
    assert_eq!(
        full.len(),
        in_scope.len(),
        "one row per class in both files"
    );

    let (mut rows, mut kept) = (0usize, 0usize);
    for (i, (line, (class, want_rows, want_kept))) in
        in_scope.iter().zip(SHAPE_IN_SCOPE.iter()).enumerate()
    {
        let head = format!("{class}: rows={want_rows} rows_in_scope={want_kept} ");
        assert!(
            line.starts_with(&head),
            "shape_in_scope.txt row {i}: {line:?} does not start with {head:?}"
        );
        assert!(
            full[i].starts_with(&format!("{class}: ")),
            "shape_in_scope.txt row {i} is {class}, shape.txt row {i} is {:?} — the two files \
             must stay in the same class order",
            full[i]
        );
        // The gap identity (which side is missing which property) is copied
        // from shape.txt and must not drift from it.
        let tail = |s: &str| {
            s.split_once("oracle_only=")
                .map(|(_, t)| t.to_string())
                .unwrap_or_default()
        };
        assert_eq!(
            tail(line),
            tail(&full[i]),
            "shape_in_scope.txt row {i} ({class}) reports a different gap than shape.txt"
        );
        rows += want_rows;
        kept += want_kept;
    }
    assert_eq!(rows, 429, "shape rows in the full census");
    assert_eq!(
        kept, 149,
        "shape rows on r4133-gating cases (WP-RP1's 149 -> 0)"
    );
}

/// RP1.1's deliverable, read back against the evidence that motivated it: the
/// live Generator and Sensor property tables now have exactly the length
/// `shape.txt` records for the r4133 oracle, and carry the names that file lists
/// as `oracle_only`.
///
/// This is the offline half of "the generator `shape_count` gap is closed" — the
/// live half is the census knob on the r4133 channel, which cannot run in plain
/// `cargo test`. It is deliberately driven off the vendored file rather than
/// literal counts, so a re-measurement that moved `shape.txt` moves this
/// assertion with it instead of leaving a stale number behind.
#[test]
fn rp1_1_closes_the_generator_and_sensor_shape_gaps() {
    let rows = data_rows("shape.txt", None);

    // Read the tables through the same surface the census does: the live
    // property list of a real element (`AllPropertyNames` on the oracle side,
    // `Dss::element_properties` here).
    let mut dss = dss_core::exec::Dss::new();
    for cmd in [
        "New Circuit.shapeprobe",
        "New Generator.g1 bus1=b1 phases=3 kv=12.47 kw=100",
        "New Line.l1 bus1=b1 bus2=b2 phases=3 length=1",
        "New Sensor.s1 element=Line.l1 terminal=1 kvbase=12.47",
    ] {
        dss.command(cmd);
        assert!(dss.errors().is_empty(), "`{cmd}` -> {:?}", dss.errors());
    }
    let element_of = |class: &str| match class {
        "generator" => "Generator.g1",
        "sensor" => "Sensor.s1",
        other => panic!("no probe element for {other}"),
    };

    for (class, want_oracle_count, want_missing) in RP1_1_CLOSED {
        let line = rows
            .iter()
            .find(|l| l.starts_with(&format!("{class}: ")))
            .unwrap_or_else(|| panic!("shape.txt has no {class} row"));

        // The census's own numbers, re-read rather than restated.
        let field = |key: &str| -> usize {
            line.split_whitespace()
                .find_map(|t| t.strip_prefix(key))
                .unwrap_or_else(|| panic!("{class}: shape.txt row has no {key}"))
                .parse()
                .unwrap_or_else(|e| panic!("{class}: bad {key} in shape.txt: {e}"))
        };
        let oracle_count = field("oracle_count=");
        assert_eq!(
            oracle_count, *want_oracle_count,
            "{class}: shape.txt's oracle_count moved — RP1.1 sized the port's \
             table on it"
        );
        assert!(
            field("rust_count=") < oracle_count,
            "{class}: shape.txt must still record the PRE-RP1.1 rust_count (it is \
             frozen evidence, never edited in place)"
        );
        for name in *want_missing {
            assert!(
                line.contains(&format!("'{name}'")),
                "{class}: shape.txt no longer names {name:?} as oracle_only"
            );
        }

        // What the port answers today.
        let live: Vec<String> = dss
            .element_properties(element_of(class))
            .unwrap_or_else(|| panic!("{class}: probe element missing"))
            .into_iter()
            .map(|(n, _)| n.to_lowercase())
            .collect();
        assert_eq!(
            live.len(),
            oracle_count,
            "{class}: the port's table is {} names long, r4133's is {oracle_count} \
             — the shape gap this sub-step closes is back",
            live.len()
        );
        for name in *want_missing {
            assert!(
                live.contains(&(*name).to_string()),
                "{class}: the port's table still lacks {name:?}"
            );
        }
    }
}

/// RP1.4's deliverable, read back the same way — but this shape row runs
/// **backwards**, so what must hold is the opposite of the test above: the port
/// KEEPS the property, and the r4133 side is the one that lost it.
///
/// `shape.txt` records `gendispatcher: rust_count=10 oracle_count=9
/// oracle_only=[] rust_only=['weights']` — r4133's `TGenDispatcher` names seven
/// properties but declares six (`Version8/Source/Controls/GenDispatcher.pas:92,133`),
/// so `TCktElementClass.DefineProperties` overwrites slot 7 with `basefreq`
/// (`Common/CktElementClass.pas:98`). RP1.4 closes the gap with a `PROPS_015X`
/// row, NOT by touching the engine, and the failure mode that row would hide is
/// exactly a port that "fixed" the shape by dropping or renaming `Weights`.
/// This pins that it did not: 10 names, `weights` still at slot 7, still after
/// `genlist` and ahead of the inherited tail — the ordering r4133 intended and
/// dss_capi has. The row's own inertness on a capture that knows the name is
/// pinned in the harness
/// (`props_015x_tests::shipped_gendispatcher_weights_row_is_inert_when_the_oracle_knows_it`).
#[test]
fn rp1_4_keeps_the_gendispatcher_weights_the_r4133_table_loses() {
    let rows = data_rows("shape.txt", None);
    let line = rows
        .iter()
        .find(|l| l.starts_with("gendispatcher: "))
        .expect("shape.txt has no gendispatcher row");
    assert!(
        line.contains("oracle_only=[]") && line.contains("rust_only=['weights']"),
        "shape.txt's gendispatcher gap changed direction: {line:?}"
    );
    let field = |key: &str| -> usize {
        line.split_whitespace()
            .find_map(|t| t.strip_prefix(key))
            .unwrap_or_else(|| panic!("gendispatcher: shape.txt row has no {key}"))
            .parse()
            .unwrap_or_else(|e| panic!("gendispatcher: bad {key} in shape.txt: {e}"))
    };
    let (rust_count, oracle_count) = (field("rust_count="), field("oracle_count="));
    assert_eq!(
        rust_count,
        oracle_count + 1,
        "the census recorded exactly one Rust-side extra"
    );

    let mut dss = dss_core::exec::Dss::new();
    for cmd in [
        "New Circuit.gdshapeprobe",
        "New Line.l1 bus1=b1 bus2=b2 phases=3 length=1",
        "New Generator.g1 bus1=b2 phases=3 kv=12.47 kw=100",
        "New GenDispatcher.gd1 element=Line.l1 terminal=1 kwlimit=50 genlist=[g1]",
    ] {
        dss.command(cmd);
        assert!(dss.errors().is_empty(), "`{cmd}` -> {:?}", dss.errors());
    }
    let live: Vec<String> = dss
        .element_properties("GenDispatcher.gd1")
        .expect("probe element missing")
        .into_iter()
        .map(|(n, _)| n.to_lowercase())
        .collect();
    assert_eq!(
        live.len(),
        rust_count,
        "the port's GenDispatcher table is {} names long, the census recorded \
         {rust_count} — RP1.4 changes no engine code, so this must not move",
        live.len()
    );
    assert_eq!(
        live.iter().position(|n| n == "weights"),
        Some(6),
        "`weights` must stay at property 7 (0-based 6), right after `genlist` \
         and ahead of the inherited basefreq/enabled/like tail: {live:?}"
    );
    assert_eq!(
        &live[5..],
        &["genlist", "weights", "basefreq", "enabled", "like"],
        "the GenDispatcher tail changed shape"
    );
}

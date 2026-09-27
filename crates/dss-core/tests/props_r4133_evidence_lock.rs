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
//! directory's `README.md`), so this file is the guard — and what stops a quiet
//! edit is the **digest** of item 1, not the structural checks of items 2-3: an
//! in-place rewrite of a value in `examples_full.txt`'s 2 203 numeric-only rows
//! keeps every count, sum and bin of those items (RETRO_FIXES RF-D00-16).
//!
//! # What is locked
//!
//! 1. **Bytes** — SHA-256 + length over the five verbatim copies
//!    ([`VERBATIM`]) and over the six derived files ([`DERIVED`]): the four the
//!    replay consumes (`examples_full.txt`, `examples_supplement.txt`,
//!    `bins.tsv`, `shape_in_scope.txt`) and the two in-scope pair extracts
//!    (`structural_pairs_in_scope.txt`, `numeric_pairs_in_scope.txt`). They
//!    are hashed **raw**: `.gitattributes` marks the directory `-text`
//!    (asserted below), so the committed bytes and the working-tree bytes are
//!    the same on every platform, CRLF and LF files alike.
//! 2. **Row counts** of the derived files (the acceptance criterion of WP-RP0).
//! 3. **Cross-file equalities** — `bins.tsv` ↔ the pair files ↔
//!    `examples_full.txt` ↔ the in-scope pair files ↔ `shape_in_scope.txt`,
//!    including every value column: both `max_rel` columns against their pair
//!    files and against the numeric bin rule (`bin = 7` iff `max_rel >= 1e-4`),
//!    and every `(pair, rust-example, r4133-example)` triple of the two
//!    in-scope extracts as a row of the digest-locked `examples_full.txt`.
//!    These catch a partial regeneration and tie the files to each other;
//!    what pins a value the triple check admits (another recorded spelling of
//!    the same pair) is the digest of item 1.
//! 4. **The two counted claims the README's data traps make** — the 17
//!    heterogeneous structural pairs and bin 1's nine echo-carrying pairs (four
//!    mixed, five pure echo). Both are re-derived here from
//!    `examples_full.txt` + `bins.tsv`, so the corrected numbers are machine-
//!    checked rather than prose.
//! 5. **The RP2.1 supplement** — `examples_supplement.txt`, the measured
//!    spelling inventory of the 26 pairs no frozen row can carry (24 created by
//!    the WP-RP1 shape closures, plus the pair the 2026-08-08 walk missed and
//!    the pair a `SKIP_PROPS` row hid). Row/pair/cell counts, a unique
//!    `(pair, rust, r4133)` triple, disjointness from `bins.tsv`, the
//!    presence of the provenance header that IS those pairs' bin assignment,
//!    and its machine-readable `# BIN <pair> <bin>` lines ([`SUPPLEMENT_BINS`]).
//! 6. **The population the in-scope numbers measure** — the live manifests'
//!    `engines ∈ {both, r4133}` cases are the census's 462 ([`CENSUS_IN_SCOPE`],
//!    count + digest) plus the named post-census additions
//!    ([`IN_SCOPE_ADDED_SINCE_CENSUS`]), no census-time `capi_v0145` case
//!    ([`CENSUS_OUT_OF_SCOPE`], by name, the list itself locked by
//!    [`CENSUS_ALL`]) is in scope or named as an addition, [`MANIFESTS`] are
//!    the manifests the gate's population lock counts, and the r4133
//!    `kind: "skip"` rows of `tests/corpus/ledger.json` the README's in-scope
//!    section counts are still the ones it names ([`R4133_SKIPS`]). These read
//!    live files, so they are the tripwire for the README's population prose
//!    (RETRO_FIXES RF-D00-16).
//!
//! Nothing here needs an oracle, a solve or a feature flag: it is a data lock,
//! green in both lanes.
//!
//! A deliberate re-measurement (the RP0.2 knob) that legitimately moves these
//! files updates the constants below in the same commit — that diff is the
//! review artifact.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use dss_test_harness::harness::{ValueVerdict, value_verdict};
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

/// The six derived files: name, SHA-256 over the raw bytes, byte length.
/// Unlike [`VERBATIM`] they have no local source to be a copy of —
/// `examples_full.txt`, `bins.tsv`, `shape_in_scope.txt` and the two in-scope
/// pair extracts are WP-RP0's derivations of the 2026-08-08 census,
/// `examples_supplement.txt` is RP2.1's measurement — so the digest is the only
/// thing that pins their value text: the structural checks below hold across
/// an in-place value rewrite (RETRO_FIXES RF-D00-16, finding
/// `RP|RP0.1|AC1|AC1-1`), and an in-scope extract's example cell can be
/// re-pointed at another spelling `examples_full.txt` records for the same
/// pair without breaking any count or triple (the settlement's finding AT-2:
/// 112 of the 198 structural and 39 of the 53 numeric in-scope pairs have
/// more than one).
const DERIVED: &[(&str, &str, usize)] = &[
    (
        "examples_full.txt",
        "921e37d5c9b8e8ccc6be9b877e68bb4d176be77435cca59aef45da227cd72e43",
        173_902,
    ),
    (
        "examples_supplement.txt",
        "b2e2c184730e4420a1aaf13d5be9ee9e0fbec39ad3a1ecb53109b5942b7e3d00",
        11_748,
    ),
    (
        "bins.tsv",
        "6125a554a033fa5527abebaaae86a0fb408ed202f8a11bbcba885e562910f40f",
        13_921,
    ),
    (
        "shape_in_scope.txt",
        "cb0c7e068dfc01a0d5bca4dbcb024e7f0dc1a7b72146018dc69034724593deb4",
        382,
    ),
    (
        "structural_pairs_in_scope.txt",
        "fccfc39fcbd121775824525149d33db9348370d1bb440684a426461ea585f198",
        8903,
    ),
    (
        "numeric_pairs_in_scope.txt",
        "e1c858f6a8dbe96219781964d43eda78aa01e41bf807bc6368ba3d42dc5a7684",
        3295,
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
/// The supplement header's `# BIN <pair> <bin>` lines: the two pairs no README
/// WP-RP1 record bins (the replay reads the README's prose binning only for
/// the 24 WP-RP1 pairs), declared by the file itself and checked against their
/// own data rows by the README's rule in
/// [`the_supplement_carries_only_pairs_no_frozen_row_can`]:
/// `regcontrol.fwdthreshold` echoes prop 36
/// (`Version8/Source/Controls/RegControl.pas:294`), which `InitPropertyValues`
/// never writes (`:1423-1459` initialises 1..32 only);
/// `regcontrol.revthreshold` echoes the `PropertyValue[23]` store through the
/// getter's fallthrough (`:820-827`) — `:1448`'s default `'100'`, or the
/// deck's own token stored by the `Edit` loop at `:420`.
const SUPPLEMENT_BINS: &[(&str, u8)] = &[
    ("regcontrol.fwdthreshold", 5),
    ("regcontrol.revthreshold", 7),
];

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

/// The four manifests the in-scope filter joins census `case` labels to, as
/// `(label prefix, repo-root-relative path)`: a label is `"<prefix>:<path>"`
/// (vendored `README.md` §"The in-scope filter").
const MANIFESTS: &[(&str, &str)] = &[
    ("solvable_now", "tests/corpus/manifests/solvable_now.json"),
    ("asymmetric", "tests/corpus/asymmetric/manifest.json"),
    ("controls", "tests/corpus/controls/manifest.json"),
    ("modes", "tests/corpus/modes/manifest.json"),
];

/// **The population every in-scope number here measures**: the census-time
/// cases of [`MANIFESTS`] with `engines ∈ {both, r4133}` — 462 of 521 — as
/// `(count, digest)`, the digest being the SHA-256 over the sorted labels,
/// each followed by `\n`.
/// `cells_in_scope`, `max_rel_in_scope`, the two `*_in_scope.txt` extracts and
/// `shape_in_scope.txt` are all measured over exactly these cases, and
/// `props_r4133_replay.rs` uses `max_rel_in_scope` as a ceiling. Re-derived
/// 2026-09-27 (RETRO_FIXES RF-D00-16, finding `RP|RP0.1|AT2|AT2-3`) by applying
/// the filter to the four manifests as RP0.1's vendoring commit `6db7f202`
/// holds them (`git show 6db7f202:<manifest>`): 521 cases = 366 `both` + 96
/// `r4133` + 59 `capi_v0145`.
const CENSUS_IN_SCOPE: (usize, &str) = (
    462,
    "4bb3fa6ff02c9ba8f1581c42f14cb394134b17b0849cddd910ce286464418e03",
);

/// In-scope cases the manifests gained after the census, by label: decks the
/// census never saw. None of them has a frozen cell, so no in-scope number
/// above describes them: they are named here so that the population lock can
/// tell a known addition from drift. A census-time case is never an addition
/// — one of [`CENSUS_OUT_OF_SCOPE`] named here reds. At RF-D00-16 the live
/// manifests hold 526 cases (366 `both` + 101 `r4133` + 59 `capi_v0145`), 467
/// in scope = the census's 462 + these five; the other change since, three GIC
/// decks moved `both` → `r4133`, stays in scope and moves nothing.
const IN_SCOPE_ADDED_SINCE_CENSUS: &[&str] = &[
    "asymmetric:autotrans/autotrans_xfmrcode.dss",
    "controls:energymeter/midi_relcalc.dss",
    "controls:espvlcontrol/espvlcontrol.dss",
    "modes:faultstudy/faultstudy_micro.dss",
    "modes:makeposseq/makeposseq_gic.dss",
];

/// The census-time cases of [`MANIFESTS`] the in-scope filter left out — the
/// 59 `capi_v0145` cases of the 521, sorted, re-derived like
/// [`CENSUS_IN_SCOPE`] from `git show 6db7f202:<manifest>` (RETRO_FIXES
/// RF-D00-16 settlement, findings AT-1/AC-1/AT-4). Their census cells were
/// measured *outside* the in-scope population, so none of them may join it
/// unnoticed: a flip to `both`/`r4133` reds with its label, and naming it in
/// [`IN_SCOPE_ADDED_SINCE_CENSUS`] cannot green that red (a subtraction of
/// the named additions alone would restore the census digest exactly). All 59
/// are still `capi_v0145` at RF-D00-16.
const CENSUS_OUT_OF_SCOPE: &[&str] = &[
    "controls:autotrans/autotrans_both.dss",
    "controls:autotrans/autotrans_reg.dss",
    "controls:autotrans/midi_autotrans.dss",
    "controls:autotrans/midi_autotrans_both.dss",
    "controls:capcontrol/capcontrol_follow_noshape.dss",
    "controls:capcontrol/capcontrol_pf.dss",
    "controls:combo/combo_metering.dss",
    "controls:combo/midi_controls.dss",
    "controls:gendispatcher/gendispatcher.dss",
    "controls:gendispatcher/gendispatcher_kvarlimit.dss",
    "controls:gendispatcher/midi_gendispatcher.dss",
    "controls:invcontrol/invcontrol_avr.dss",
    "controls:invcontrol/invcontrol_drc.dss",
    "controls:invcontrol/invcontrol_expmodel.dss",
    "controls:invcontrol/invcontrol_monbus.dss",
    "controls:invcontrol/invcontrol_storage_vv_vw.dss",
    "controls:invcontrol/invcontrol_vv_drc.dss",
    "controls:invcontrol/invcontrol_wattpf.dss",
    "controls:invcontrol/invcontrol_wattvar.dss",
    "controls:invcontrol/midi_invcontrol_drc.dss",
    "controls:sensor/midi_sensor.dss",
    "controls:sensor/sensor_map.dss",
    "controls:storagecontroller/storagectrl_follow.dss",
    "controls:storagecontroller/storagectrl_time.dss",
    "controls:swtcontrol/swtcontrol_lock.dss",
    "modes:inputformat/shape_filearr/shape_filearr.dss",
    "modes:inputformat/shape_mmf/shape_mmf.dss",
    "modes:inputformat/shape_mmf_io/shape_mmf_io.dss",
    "modes:makeposseq/makeposseq_ctrl.dss",
    "modes:makeposseq/makeposseq_line.dss",
    "modes:makeposseq/makeposseq_pc.dss",
    "modes:makeposseq/makeposseq_report.dss",
    "modes:makeposseq/makeposseq_shunt.dss",
    "modes:makeposseq/makeposseq_xfmr.dss",
    "modes:reduce/midi_reduce.dss",
    "modes:reduce/reduce_default.dss",
    "modes:reduce/reduce_keeplist.dss",
    "modes:reduce/reduce_mergeparallel.dss",
    "modes:reduce/reduce_shortlines.dss",
    "modes:reduce/reduce_switches.dss",
    "solvable_now:Test/CapControlFollow.dss",
    "solvable_now:Version8/Distrib/EPRITestCircuits/ckt24/Run_Ckt24.dss",
    "solvable_now:Version8/Distrib/EPRITestCircuits/ckt24/master_ckt24.dss",
    "solvable_now:Version8/Distrib/Examples/ADiakoptics/IEEE_123_Bus-G/Torn_Circuit/Master_Interconnected.dss",
    "solvable_now:Version8/Distrib/Examples/ADiakoptics/ckt24/Torn_Circuit/Master.DSS",
    "solvable_now:Version8/Distrib/Examples/ADiakoptics/ckt24/Torn_Circuit/Master_Interconnected.dss",
    "solvable_now:Version8/Distrib/Examples/HarmonicsTMode/IEEE_519.DSS",
    "solvable_now:Version8/Distrib/Examples/HarmonicsVariableLoad/IEEE_519.DSS",
    "solvable_now:Version8/Distrib/Examples/Matlab/HarmonicT_MATLAB/IEEE_519.DSS",
    "solvable_now:Version8/Distrib/Examples/MemoryMappingLoadShapes/ckt24/master_ckt24-mm-csv-pq.dss",
    "solvable_now:Version8/Distrib/Examples/MemoryMappingLoadShapes/ckt24/master_ckt24-mm-dbl-p.dss",
    "solvable_now:Version8/Distrib/Examples/MemoryMappingLoadShapes/ckt24/master_ckt24-mm-sng-p.dss",
    "solvable_now:Version8/Distrib/Examples/MemoryMappingLoadShapes/ckt24/master_ckt24-mm-txt-p.dss",
    "solvable_now:Version8/Distrib/Examples/MemoryMappingLoadShapes/ckt24/master_ckt24-mm-txt-pq.dss",
    "solvable_now:Version8/Distrib/Examples/MemoryMappingLoadShapes/ckt24/master_ckt24-nomm.dss",
    "solvable_now:Version8/Distrib/Examples/MemoryMappingLoadShapes/ckt24/master_ckt24.dss",
    "solvable_now:Version8/Distrib/Examples/Microgrid/GridFormingInverter/GFM_IEEE8500/Run_RecloserSiting.DSS",
    "solvable_now:Version8/Distrib/Examples/Paulo_Example/DSSFiles/subestacao.dss",
    "solvable_now:Version8/Distrib/IEEETestCases/8500-Node/Run_RecloserSiting.DSS",
];

/// All 521 census-time labels of [`MANIFESTS`], any `engines`, as `(count,
/// digest)` in [`CENSUS_IN_SCOPE`]'s form (`git show 6db7f202:<manifest>`,
/// 34 258 bytes hashed). The census in-scope set and [`CENSUS_OUT_OF_SCOPE`]
/// must add up to it, so that list cannot lose or gain a label unnoticed.
const CENSUS_ALL: (usize, &str) = (
    521,
    "050cef2ecb04569f5fc76219e2185644913b2f2d98174d14dea70fa68cf24d71",
);

/// Every r4133-channel `kind: "skip"` row of `tests/corpus/ledger.json`, as
/// `(case label, ledger id, cause_ref)`: in-scope cases whose r4133 side never
/// runs, so the `engines`-only filter admits cells the r4133 compare cannot
/// reach. The README's "four of the 462" are the `epri-303-crash` rows (all
/// census cases); the fifth is the post-census `espvlcontrol` deck, whose
/// class r4133 cannot instantiate.
const R4133_SKIPS: &[(&str, &str, &str)] = &[
    (
        "asymmetric:line/line_spacing_asym.dss",
        "r4133-linespacing-asym-303",
        "epri-303-crash",
    ),
    (
        "controls:espvlcontrol/espvlcontrol.dss",
        "r4133-espvlcontrol-uninstantiable",
        "epri-espvlcontrol-uninstantiable",
    ),
    (
        "modes:inputformat/shape_binfiles/shape_binfiles.dss",
        "r4133-binaryshape-303",
        "epri-303-crash",
    ),
    (
        "solvable_now:Test/IEEE13_LineAndCableSpacing.dss",
        "r4133-linecablespacing-303",
        "epri-303-crash",
    ),
    (
        "solvable_now:Test/IEEE13_LineSpacing.dss",
        "r4133-linespacing-303",
        "epri-303-crash",
    ),
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
    max_rel: String,
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
                max_rel: f[6].to_string(),
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

/// A `max_rel` cell of `bins.tsv` (`%.2e`, README §"Known data traps"):
/// finite, >= 0.
fn parse_max_rel(pair: &str, column: &str, cell: &str) -> f64 {
    let v: f64 = cell
        .parse()
        .unwrap_or_else(|e| panic!("bins.tsv: {pair} {column} {cell:?}: {e}"));
    assert!(
        v.is_finite() && v >= 0.0,
        "bins.tsv: {pair} {column} {cell:?} is not a relative error"
    );
    v
}

/// The `(pair, rust, r4133)` join keys of the digest-locked
/// `examples_full.txt` (unique — [`examples_full_carries_every_pair_and_every_cell`]).
fn examples_full_triples() -> BTreeSet<(String, String, String)> {
    data_rows(
        "examples_full.txt",
        Some("class.prop | rust | r4133 | count"),
    )
    .iter()
    .map(|line| {
        let (pair, rust, r4133, _) = parse_row("examples_full.txt", line);
        (pair, rust, r4133)
    })
    .collect()
}

/// Raw bytes of one file of [`DIR`]: `(length, SHA-256 hex)`.
fn digest(name: &str) -> (usize, String) {
    let path = repo_root().join(DIR).join(name);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    (bytes.len(), format!("{:x}", Sha256::digest(&bytes)))
}

/// A repo-root-relative JSON file, parsed.
fn read_json(rel: &str) -> serde_json::Value {
    let path = repo_root().join(rel);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

/// The live manifests' cases as `"<prefix>:<path>" -> engines`, every case of
/// [`MANIFESTS`]. A case with no `engines` key is `both`, the corpus schema's
/// default the gate applies (`corpus_gate/manifest.rs`, `default_engines`;
/// TESTING.md names `"both"` the default), so such a case is classified exactly
/// as the gate channels it; an `engines` that is not a string, an unknown
/// channel or a case listed twice panics — the join must be exact.
fn manifest_engines() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (prefix, rel) in MANIFESTS {
        let json = read_json(rel);
        let cases = json["cases"]
            .as_array()
            .unwrap_or_else(|| panic!("{rel}: no `cases` array"));
        assert!(!cases.is_empty(), "{rel}: no case");
        for case in cases {
            let path = case["path"]
                .as_str()
                .unwrap_or_else(|| panic!("{rel}: a case without a `path`: {case}"));
            let label = format!("{prefix}:{path}");
            let engines = case.get("engines").map_or("both", |e| {
                e.as_str()
                    .unwrap_or_else(|| panic!("{label}: `engines` is not a string: {e}"))
            });
            assert!(
                matches!(engines, "both" | "r4133" | "capi_v0145"),
                "{label}: unknown `engines` {engines:?}"
            );
            assert!(
                out.insert(label.clone(), engines.to_string()).is_none(),
                "{label}: listed twice"
            );
        }
    }
    out
}

/// SHA-256 hex over labels in ascending order, each followed by `\n` — the
/// form of [`CENSUS_IN_SCOPE`] and [`CENSUS_ALL`].
fn label_digest(labels: &BTreeSet<&str>) -> String {
    let blob: String = labels.iter().map(|label| format!("{label}\n")).collect();
    format!("{:x}", Sha256::digest(blob.as_bytes()))
}

/// The in-scope filter of the vendored `README.md`: `engines ∈ {both, r4133}`.
fn in_scope(engines: &str) -> bool {
    engines == "both" || engines == "r4133"
}

#[test]
fn verbatim_copies_keep_their_bytes() {
    let mut total = 0usize;
    for (name, want, len) in VERBATIM {
        let (got_len, got) = digest(name);
        assert_eq!(
            got_len, *len,
            "{name}: {got_len} bytes, expected {len} — this is a verbatim copy of a local-only \
             source; it is never edited in place"
        );
        assert_eq!(
            got, *want,
            "{name}: content digest moved. The five copies in {DIR} are byte-identical to the \
             G1.1 extracts in the (gitignored) investigations/g1_1_r4133_props/; if a \
             re-measurement legitimately replaced them, update VERBATIM in the same commit."
        );
        total += got_len;
    }
    assert_eq!(total, 24_944, "the five copies total 24 944 bytes");
}

/// **The digest is what stops a quiet edit of a derived file** ([`DERIVED`]).
/// The row counts, sums and bins below hold across an in-place rewrite of a
/// value; one moved byte anywhere — a value, a count, a comment line of the
/// supplement's header — moves the digest. That includes the two in-scope
/// extracts: [`bins_tsv_agrees_with_the_pair_files`] ties their columns to the
/// other files, but an example cell re-pointed at another spelling
/// `examples_full.txt` records for the same pair passes it — only the digest
/// sees that edit.
#[test]
fn derived_files_keep_their_bytes() {
    for (name, want, len) in DERIVED {
        let (got_len, got) = digest(name);
        assert_eq!(
            got_len, *len,
            "{name}: {got_len} bytes, expected {len} — a derived evidence file moves only \
             together with DERIVED"
        );
        assert_eq!(
            got, *want,
            "{name}: content digest moved. A deliberate re-measurement (the RP0.2 knob, or a \
             supplement re-measure) that legitimately replaces this file updates DERIVED in the \
             same commit — that diff is the review artifact; a hand edit of a value is what \
             this assertion exists to stop."
        );
    }
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
                 {SUPPLEMENT} carries comments, so this is a hand edit of the evidence \
                 or a generator change"
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

    // The in-scope extracts' example columns must name a spelling the
    // digest-locked census extract recorded, on the same side (RF-D00-16).
    let full = examples_full_triples();
    let (mut max_rel_full, mut max_rel_in_scope, mut triples) = (0usize, 0usize, 0usize);

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
            let (pair, rust, r4133, tail) = parse_row(file, &line);
            // Numeric files carry `max_rel | rows`, structural ones `rows`.
            let (max_rel, rows) = if rows_idx == 1 {
                let (m, r) = tail
                    .split_once(" | ")
                    .unwrap_or_else(|| panic!("{file}: {pair} has no max_rel column: {tail:?}"));
                (Some(m), r)
            } else {
                (None, tail.as_str())
            };
            assert!(
                !rows.contains(" | "),
                "{file}: {pair} carries an extra column: {tail:?}"
            );
            let cells = rows_field(file, rows);
            let b = by_key
                .get(&(pair.clone(), kind.to_string()))
                .unwrap_or_else(|| panic!("{file}: {pair} has no {kind} row in bins.tsv"));
            let want = if in_scope { b.cells_in_scope } else { b.cells };
            assert_eq!(
                cells, want,
                "{file}: {pair} carries {cells} rows, bins.tsv says {want}"
            );
            if let Some(max_rel) = max_rel {
                let (column, want) = if in_scope {
                    ("max_rel_in_scope", &b.max_rel_in_scope)
                } else {
                    ("max_rel", &b.max_rel)
                };
                assert_eq!(
                    max_rel, want,
                    "{file}: {pair} max_rel {max_rel}, bins.tsv {column} says {want}"
                );
                if in_scope {
                    max_rel_in_scope += 1;
                } else {
                    max_rel_full += 1;
                }
            }
            if in_scope {
                assert!(
                    full.contains(&(pair.clone(), rust.clone(), r4133.clone())),
                    "{file}: {pair}'s example ('{rust}' | '{r4133}') is no row of \
                     examples_full.txt — the extract names a spelling the census never recorded, \
                     or has its rust/r4133 sides swapped"
                );
                triples += 1;
            }
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
    // Non-vacuous: every numeric row of both files and every in-scope row was
    // value-checked (94 + 53 max_rel cells, 198 + 53 example triples).
    assert_eq!(
        (max_rel_full, max_rel_in_scope, triples),
        (94, 53, 251),
        "value columns checked (numeric max_rel, in-scope max_rel, in-scope triples)"
    );
}

/// `R4133_PROPS_PLAN.md` §1.1's numeric rule on the numbers that carry it
/// (README §"Numeric pairs (bins 6–7)"): `bin = 7` iff the pair's full-census
/// `max_rel` is `>= 1e-4`, else `bin = 6`. The in-scope maximum is taken over a
/// subset of the same cells, so it never exceeds the full one, and it is `-`
/// exactly when the pair has no in-scope cell; structural rows carry `-` in
/// both columns. The replay reads both columns as ceilings
/// (`props_r4133_replay.rs`: `max_rel_in_scope` through `PairEvidence`, the
/// full `max_rel` through `frozen_max_rel`), so an inflated cell would loosen a
/// comparison silently: the pair files agree with them cell for cell
/// ([`bins_tsv_agrees_with_the_pair_files`]), this rule pins the bin, and the
/// digest ([`DERIVED`]) pins the bytes.
#[test]
fn numeric_bins_follow_their_max_rel() {
    let (mut numeric, mut in_scope) = (0usize, 0usize);
    for b in bins() {
        if b.kind == "structural" {
            assert_eq!(
                (b.max_rel.as_str(), b.max_rel_in_scope.as_str()),
                ("-", "-"),
                "bins.tsv: structural {} carries a max_rel",
                b.pair
            );
            continue;
        }
        numeric += 1;
        let max_rel = parse_max_rel(&b.pair, "max_rel", &b.max_rel);
        assert_eq!(
            b.bin == 7,
            max_rel >= 1e-4,
            "bins.tsv: {} is bin {} with max_rel {} — bin 7 iff max_rel >= 1e-4, else bin 6",
            b.pair,
            b.bin,
            b.max_rel
        );
        if b.cells_in_scope == 0 {
            assert_eq!(
                b.max_rel_in_scope, "-",
                "bins.tsv: {} has no in-scope cell but a max_rel_in_scope",
                b.pair
            );
        } else {
            in_scope += 1;
            let m = parse_max_rel(&b.pair, "max_rel_in_scope", &b.max_rel_in_scope);
            assert!(
                m <= max_rel,
                "bins.tsv: {} max_rel_in_scope {} exceeds its full-census max_rel {}",
                b.pair,
                b.max_rel_in_scope,
                b.max_rel
            );
        }
    }
    assert_eq!(
        (numeric, in_scope),
        (94, 53),
        "numeric pairs (all, in scope)"
    );
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
    let mut spellings: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
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
            triples.insert((pair.clone(), rust.clone(), r4133.clone())),
            "examples_supplement.txt: duplicate (pair, rust, r4133) triple in {line:?}"
        );
        spellings
            .entry(pair.clone())
            .or_default()
            .push((rust, r4133));
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

    // Its machine-readable form: one `# BIN <pair> <bin>` line per pair the
    // README does not bin, each naming a pair with data rows in the file.
    let declared: Vec<(String, u8)> = text
        .lines()
        .filter_map(|l| l.trim_end_matches('\r').strip_prefix("# BIN "))
        .map(|rest| {
            let (pair, bin) = rest
                .split_once(' ')
                .unwrap_or_else(|| panic!("examples_supplement.txt: bad `# BIN` line {rest:?}"));
            let bin: u8 = bin
                .parse()
                .unwrap_or_else(|e| panic!("examples_supplement.txt: `# BIN {rest}`: {e}"));
            assert!(
                (1..=7).contains(&bin) && pairs.contains_key(pair),
                "examples_supplement.txt: `# BIN {rest}` names no bin or no data-row pair"
            );
            (pair.to_string(), bin)
        })
        .collect();
    let want: Vec<(String, u8)> = SUPPLEMENT_BINS
        .iter()
        .map(|(pair, bin)| (pair.to_string(), *bin))
        .collect();
    assert_eq!(declared, want, "examples_supplement.txt `# BIN` lines");

    // A declared bin is the one the README's assignment rule (§"`bins.tsv` —
    // the assignment rule") gives the pair's own data rows, so the declaration
    // is checked, not trusted (RETRO_FIXES RF-D00-16 settlement, finding
    // AC-6). Numeric — every row `Numeric` under the census's own comparator
    // (`harness::value_verdict`, at a zero floor: these rows are divergent by
    // construction) — is bin 7 iff the pair's `max_rel` is >= 1e-4, else 6.
    // Structural is the chain of [`classify`]; the supplement orders a pair's
    // rows by count, not census order, so there is no representative row to
    // pick and every row must land in the declared bin.
    for (pair, bin) in &declared {
        let rows = &spellings[pair];
        let rels: Vec<f64> = rows
            .iter()
            .filter_map(|(rust, r4133)| match value_verdict(rust, r4133, 0.0, 0.0) {
                ValueVerdict::Numeric { max_rel, .. } => Some(max_rel),
                _ => None,
            })
            .collect();
        let derived: BTreeSet<u8> = if rels.len() == rows.len() {
            let max_rel = rels.iter().copied().fold(0.0f64, f64::max);
            BTreeSet::from([if max_rel >= 1e-4 { 7 } else { 6 }])
        } else {
            assert!(
                rels.is_empty(),
                "examples_supplement.txt: {pair} mixes numeric and structural rows {rows:?}"
            );
            rows.iter()
                .map(|(rust, r4133)| classify(rust, r4133))
                .collect()
        };
        assert_eq!(
            derived,
            BTreeSet::from([*bin]),
            "examples_supplement.txt `# BIN {pair} {bin}`: the README's rule puts its data rows \
             {rows:?} in bin(s) {derived:?}"
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

/// **The in-scope numbers still describe the population they were measured
/// on.** Every in-scope figure in this directory is a measurement over the
/// census's 462 in-scope cases ([`CENSUS_IN_SCOPE`]), while the manifests it
/// joins to are live files that keep growing. So, instead of silently changing
/// what `max_rel_in_scope` is a ceiling over:
///
/// * no census-time `capi_v0145` case ([`CENSUS_OUT_OF_SCOPE`]) is in scope
///   today or named in [`IN_SCOPE_ADDED_SINCE_CENSUS`] — its census cells were
///   measured outside the population — and either reds with its label;
/// * the live in-scope set, less the named additions, is the census set
///   exactly — same count, same digest — so a census case that left the scope
///   or a new in-scope deck nobody named reds;
/// * the census set and [`CENSUS_OUT_OF_SCOPE`] add up to the census's 521
///   labels ([`CENSUS_ALL`]), so the out-of-scope list cannot be trimmed to
///   let a flip through
///
/// (RETRO_FIXES RF-D00-16, finding `RP|RP0.1|AT2|AT2-3`, and its settlement's
/// AT-1/AC-1).
#[test]
fn the_in_scope_population_is_the_census_one_plus_named_additions() {
    let engines = manifest_engines();
    let live: BTreeSet<&str> = engines
        .iter()
        .filter(|(_, e)| in_scope(e))
        .map(|(label, _)| label.as_str())
        .collect();

    let added: BTreeSet<&str> = IN_SCOPE_ADDED_SINCE_CENSUS.iter().copied().collect();
    assert_eq!(
        added.len(),
        IN_SCOPE_ADDED_SINCE_CENSUS.len(),
        "IN_SCOPE_ADDED_SINCE_CENSUS names a case twice"
    );
    assert!(
        CENSUS_OUT_OF_SCOPE.windows(2).all(|w| w[0] < w[1]),
        "CENSUS_OUT_OF_SCOPE must be sorted and name each case once"
    );
    let out_of_scope: BTreeSet<&str> = CENSUS_OUT_OF_SCOPE.iter().copied().collect();

    let named: Vec<&str> = added.intersection(&out_of_scope).copied().collect();
    assert!(
        named.is_empty(),
        "{named:?}: census-time `capi_v0145` case(s) named in IN_SCOPE_ADDED_SINCE_CENSUS. An \
         addition is a deck the census never saw; these were walked by it and measured outside \
         the in-scope population, so naming them would hide the drift this lock exists for"
    );
    let entered: Vec<String> = out_of_scope
        .iter()
        .filter(|label| live.contains(*label))
        .map(|label| format!("{label} ({})", engines[*label]))
        .collect();
    assert!(
        entered.is_empty(),
        "census-time `capi_v0145` case(s) now in scope: {entered:?}. Their census cells were \
         measured outside the in-scope population, so `cells_in_scope`, `max_rel_in_scope` and \
         the in-scope extracts no longer cover what the r4133 channel compares: re-derive them \
         (the RP0.2 census knob) or keep the case `capi_v0145` — never re-baseline this pin"
    );
    for label in &added {
        assert!(
            live.contains(label),
            "{label}: named as an in-scope addition since the census, but the live manifests \
             have it as {}",
            engines.get(*label).map_or("no case", String::as_str)
        );
    }

    let census: BTreeSet<&str> = live.difference(&added).copied().collect();
    // Nameable drift for the message: live `capi_v0145` cases that were not
    // census-time ones — a census in-scope case that left the scope, or a new
    // out-of-scope deck (harmless, listed for completeness).
    let left: Vec<&str> = engines
        .iter()
        .filter(|(label, e)| e.as_str() == "capi_v0145" && !out_of_scope.contains(label.as_str()))
        .map(|(label, _)| label.as_str())
        .collect();
    assert_eq!(
        (census.len(), label_digest(&census).as_str()),
        CENSUS_IN_SCOPE,
        "the live in-scope population less the {} named additions is no longer the census's \
         (live: {} of {} cases in scope). A new in-scope deck has no frozen cell: name it in \
         IN_SCOPE_ADDED_SINCE_CENSUS. A census case that was removed or renamed, or that left \
         the {{both, r4133}} scope (live `capi_v0145` cases the census did not have as \
         `capi_v0145`: {left:?}), means the frozen in-scope measurements no longer cover what \
         the r4133 channel compares (diff the labels against `git show 6db7f202:<manifest>` of \
         the four MANIFESTS): re-derive them (the RP0.2 census knob), never re-baseline this pin",
        added.len(),
        live.len(),
        engines.len()
    );

    let all: BTreeSet<&str> = census.union(&out_of_scope).copied().collect();
    assert_eq!(
        (all.len(), label_digest(&all).as_str()),
        CENSUS_ALL,
        "the census in-scope set and CENSUS_OUT_OF_SCOPE no longer add up to the census's 521 \
         labels: CENSUS_OUT_OF_SCOPE is census evidence (`git show 6db7f202:<manifest>`), never \
         edited to let a case into the scope"
    );
}

/// **[`MANIFESTS`] are the gated corpus.** The population lock above sees only
/// the cases of the manifests it reads, so a gated family missing from
/// [`MANIFESTS`] would add in-scope cases it cannot see. The committed
/// fingerprint of the gated population, `tests/corpus/manifests/
/// population.lock.json` (`population_lock.rs` fails on any live count that
/// moves), records `solvable_now.json`'s case count and each family's
/// (`family_counts`): [`MANIFESTS`] must be `solvable_now` plus exactly those
/// families, at `tests/corpus/<family>/manifest.json`, with those counts
/// (RETRO_FIXES RF-D00-16 settlement, finding AC-5).
#[test]
fn the_census_manifests_are_the_gated_ones() {
    let lock = read_json("tests/corpus/manifests/population.lock.json");
    let count = |v: &serde_json::Value, what: &str| -> usize {
        v.as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or_else(|| panic!("population.lock.json: no case count for {what}"))
    };
    let mut want: BTreeMap<String, (String, usize)> = BTreeMap::new();
    want.insert(
        "solvable_now".to_string(),
        (
            "tests/corpus/manifests/solvable_now.json".to_string(),
            count(
                &lock["manifest_counts"]["solvable_now.json"],
                "solvable_now.json",
            ),
        ),
    );
    let families = lock["family_counts"]
        .as_object()
        .expect("population.lock.json: no `family_counts` object");
    assert!(!families.is_empty(), "population.lock.json: no family");
    for (family, n) in families {
        want.insert(
            family.clone(),
            (
                format!("tests/corpus/{family}/manifest.json"),
                count(n, family),
            ),
        );
    }
    let got: BTreeMap<String, (String, usize)> = MANIFESTS
        .iter()
        .map(|(prefix, rel)| {
            let cases = read_json(rel)["cases"]
                .as_array()
                .unwrap_or_else(|| panic!("{rel}: no `cases` array"))
                .len();
            (prefix.to_string(), (rel.to_string(), cases))
        })
        .collect();
    assert_eq!(
        got, want,
        "MANIFESTS (prefix -> (path, cases)) must be population.lock.json's solvable_now + \
         families: a gated manifest missing here holds in-scope cases the population lock \
         cannot see"
    );
}

/// **The r4133 skip rows the README's in-scope section names are still skip
/// rows.** The filter is `engines`-only, so it admits cases whose r4133 side
/// the ledger switches off; the README counts them (the four `epri-303-crash`
/// census cases, and since G1.2 the `espvlcontrol` deck). This pins the ledger
/// side of that prose: every r4133 `kind: "skip"` row is one of
/// [`R4133_SKIPS`] and each of those is still there with its id and cause, on
/// an in-scope `engines: both` case (the `capi_v0145` channel gates each deck
/// fully), census or named addition as the table says (RETRO_FIXES RF-D00-16,
/// finding `RP|RP0.1|AT3|AT3-1`).
#[test]
fn the_r4133_skip_rows_are_the_readme_four_plus_espvlcontrol() {
    let engines = manifest_engines();
    let ledger = read_json("tests/corpus/ledger.json");
    let entries = ledger["entries"]
        .as_array()
        .expect("tests/corpus/ledger.json: no `entries` array");
    let field = |e: &serde_json::Value, key: &str| -> String {
        e[key]
            .as_str()
            .unwrap_or_else(|| panic!("ledger.json: an entry without a `{key}` string: {e}"))
            .to_string()
    };
    let skips: BTreeSet<(String, String, String)> = entries
        .iter()
        .filter(|e| e["channel"] == "r4133" && e["kind"] == "skip")
        .map(|e| (field(e, "case"), field(e, "id"), field(e, "cause_ref")))
        .collect();
    let want: BTreeSet<(String, String, String)> = R4133_SKIPS
        .iter()
        .map(|(case, id, cause)| (case.to_string(), id.to_string(), cause.to_string()))
        .collect();
    assert_eq!(
        skips, want,
        "the r4133 skip rows of tests/corpus/ledger.json moved: the README's in-scope section \
         counts them, so update R4133_SKIPS together with a README note"
    );

    let mut crash_303 = 0usize;
    for (case, id, cause) in R4133_SKIPS {
        assert_eq!(
            engines.get(*case).map(String::as_str),
            Some("both"),
            "{id}: {case} must be an `engines: both` case of the live manifests"
        );
        let post_census = IN_SCOPE_ADDED_SINCE_CENSUS.contains(case);
        if *cause == "epri-303-crash" {
            crash_303 += 1;
            assert!(!post_census, "{id}: the four 303 skips are census cases");
        } else {
            assert!(post_census, "{id}: {case} is the post-census skip");
        }
    }
    assert_eq!(
        crash_303, 4,
        "the README names exactly four `epri-303-crash` skip cases"
    );
}

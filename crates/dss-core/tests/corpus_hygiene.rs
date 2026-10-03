//! Corpus hygiene: the files a memory-mapped LoadShape reads keep their
//! committed bytes on checkout.
//!
//! The MMF reader indexes a mapped file by its fixed record and line width, so
//! a CRLF rewrite of a text fixture changes what the engine and the live oracle
//! read. `core.autocrlf` is on in this repository, so every such fixture must be
//! declared `-text` (or `binary`) by a `.gitattributes` on its path: the
//! repo-root one for the vendored `electricdss-tst` tree, a directory-local one
//! for the synthetic decks under `tests/corpus/modes`. The declaration covers a
//! fresh checkout only: git leaves a copy it converted before the declaration
//! landed as it is, so the text fixtures, all committed with LF records, must
//! also hold no CR byte.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

/// The file arguments of every memory-mapped LoadShape command in one deck's
/// `text`, as `(argument, file)` pairs: the argument lowercased, the file as
/// written with `\` turned into `/`. The same scan as the MMF accept-set census
/// of the LoadShape unit tests.
///
/// A command is a `New`/`Edit LoadShape` line (the class name bare, quoted or
/// after `object=`) with its `~` / `more` continuation lines joined and `!` and
/// `//` comments stripped. It is memory-mapped when it sets `MemoryMapping` to
/// a value the engine reads as true (first letter `y` or `t`, optionally
/// quoted), and then each `file=`, `csvfile=`, `pqcsvfile=`, `sngfile=` and
/// `dblfile=` argument is one pair.
fn mapped_file_arguments(text: &str) -> Vec<(String, String)> {
    let definition =
        regex::Regex::new(r#"(?i)^\s*(new|edit)\s+(object\s*=\s*)?["'(\[{]?loadshape\."#)
            .expect("definition pattern");
    let mapped =
        regex::Regex::new(r#"(?i)\bmemorymapping\s*=\s*["'(\[{]?[yt]"#).expect("mm pattern");
    let argument = regex::Regex::new(
        r#"(?i)\b(file|csvfile|pqcsvfile|sngfile|dblfile)\s*=\s*["']?([^\s"')\]]+)"#,
    )
    .expect("file pattern");

    let mut commands: Vec<String> = Vec::new();
    for line in text.lines() {
        let code = line.split('!').next().unwrap_or("");
        let code = code.split("//").next().unwrap_or("").trim();
        if code.is_empty() {
            continue;
        }
        let more = code.strip_prefix('~').or_else(|| {
            code.get(..4)
                .filter(|head| head.eq_ignore_ascii_case("more"))
                .and(code.get(4..))
                .filter(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
        });
        match (more, commands.last_mut()) {
            (Some(rest), Some(last)) => {
                last.push(' ');
                last.push_str(rest);
            }
            _ => commands.push(code.to_string()),
        }
    }

    commands
        .iter()
        .filter(|c| definition.is_match(c) && mapped.is_match(c))
        .flat_map(|c| argument.captures_iter(c))
        .map(|cap| (cap[1].to_ascii_lowercase(), cap[2].replace('\\', "/")))
        .collect()
}

/// A file some memory-mapped LoadShape reads.
#[derive(Debug, Default)]
struct Fixture {
    /// The decks that read it, repo-relative and forward-slashed.
    decks: BTreeSet<String>,
    /// Read as `sngfile=` / `dblfile=` data or named `.sng` / `.dbl`.
    binary: bool,
}

/// Every file a memory-mapped LoadShape of a corpus deck reads, keyed by its
/// repo-relative forward-slashed path.
///
/// Scans every `.dss` under `tests/corpus` with [`mapped_file_arguments`] and
/// resolves each file against the deck's own directory. Redirects are not
/// followed: a redirected deck is a `.dss` file of its own and is scanned the
/// same way.
fn mapped_fixtures(root: &Path) -> BTreeMap<String, Fixture> {
    let mut found: BTreeMap<String, Fixture> = BTreeMap::new();
    let mut stack = vec![root.join("tests/corpus")];
    while let Some(dir) = stack.pop() {
        let entries =
            std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("corpus dir entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if !path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("dss"))
            {
                continue;
            }
            let deck = path
                .strip_prefix(root)
                .expect("under the repository")
                .to_string_lossy()
                .replace('\\', "/");
            let deck_dir = parent_of(&deck).to_string();
            let bytes =
                std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            let text = String::from_utf8_lossy(&bytes);

            for (key, name) in mapped_file_arguments(&text) {
                let file = join(&deck_dir, &name);
                assert!(
                    root.join(&file).is_file(),
                    "{deck}: the mapped LoadShape file {name:?} resolves to {file}, which is not \
                     a repository file"
                );
                let lower = name.to_ascii_lowercase();
                let binary = key == "sngfile"
                    || key == "dblfile"
                    || lower.ends_with(".sng")
                    || lower.ends_with(".dbl");
                let fixture = found.entry(file).or_default();
                fixture.binary |= binary;
                fixture.decks.insert(deck.clone());
            }
        }
    }
    found
}

/// The directory part of a forward-slashed path (`""` at the top).
fn parent_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// `dir` and `name` joined, both forward-slashed, with `.` and `..` folded.
fn join(dir: &str, name: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in dir.split('/').chain(name.split('/')) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    parts.join("/")
}

/// Whether `pattern` of the `.gitattributes` in `base` matches the
/// repo-relative `file`. Reads the shapes the repository uses: a bare name
/// (any level below `base`), a slashed literal path, and a literal directory
/// followed by `/**`. Any other glob that could reach `file` panics, so the
/// verdict is never a guess.
fn attr_pattern_matches(base: &str, pattern: &str, file: &str) -> bool {
    let Some(rel) = (if base.is_empty() {
        Some(file)
    } else {
        file.strip_prefix(base).and_then(|r| r.strip_prefix('/'))
    }) else {
        return false;
    };
    let pattern = pattern.strip_prefix('/').unwrap_or(pattern);
    let is_glob = |s: &str| s.contains(['*', '?', '[', '\\']);
    if let Some(dir) = pattern.strip_suffix("/**")
        && !is_glob(dir)
    {
        return rel.starts_with(&format!("{dir}/"));
    }
    if !is_glob(pattern) {
        return if pattern.contains('/') {
            rel == pattern
        } else {
            rel.rsplit('/').next() == Some(pattern)
        };
    }
    let literal = &pattern[..pattern
        .find(['*', '?', '[', '\\'])
        .expect("a glob character")];
    let could_reach = if pattern.contains('/') {
        rel.starts_with(literal)
    } else {
        rel.rsplit('/')
            .next()
            .is_some_and(|name| name.starts_with(literal))
    };
    assert!(
        !could_reach,
        "{base}/.gitattributes: the pattern {pattern:?} could match {file}, and this test does \
         not read that glob shape"
    );
    false
}

/// Whether git checks `file` out byte-for-byte: the `text` attribute is unset
/// (`-text`, or `binary`) by the last matching declaration of the
/// `.gitattributes` chain from the repository root down to the file's
/// directory, deeper files overriding shallower ones.
fn is_checked_out_verbatim(root: &Path, file: &str) -> bool {
    let mut dirs = vec![""];
    let mut at = 0;
    while let Some(i) = file[at..].find('/') {
        dirs.push(&file[..at + i]);
        at += i + 1;
    }
    let mut verbatim = false;
    for base in dirs {
        let path = if base.is_empty() {
            root.join(".gitattributes")
        } else {
            root.join(base).join(".gitattributes")
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => panic!("read {}: {e}", path.display()),
        };
        for line in text.lines() {
            let mut tokens = line.split_whitespace();
            let Some(pattern) = tokens.next() else {
                continue;
            };
            if pattern.starts_with('#') || !attr_pattern_matches(base, pattern, file) {
                continue;
            }
            for token in tokens {
                match token {
                    "-text" | "binary" => verbatim = true,
                    t if t == "text" || t == "!text" || t.starts_with("text=") => verbatim = false,
                    t if t.starts_with("eol=") => verbatim = false,
                    _ => {}
                }
            }
        }
    }
    verbatim
}

#[test]
fn every_memory_mapped_loadshape_fixture_is_checked_out_verbatim() {
    // The scan reads every spelling of a mapped LoadShape the engine accepts,
    // and nothing else.
    let probe = "\
New Loadshape.a npts=8 interval=1 MemoryMapping=Yes csvfile=a.csv
New \"LoadShape.b\" npts=8 MemoryMapping=yes mult=(file=b.txt)
New object=\"LoadShape.c\" npts=8 MemoryMapping=True pqcsvfile=c.csv
New object=LoadShape.d npts=8 interval=1
~ MemoryMapping=Y mult=(sngfile=d.sng)
New LoadShape.e npts=8 interval=1
more\tMemoryMapping=t dblfile=sub\\e.dbl
Edit LoadShape.f MemoryMapping=\"yes\" csvfile=f.csv
New LoadShape.g npts=8 MemoryMapping=No csvfile=g.csv
New LoadShape.h npts=8 MemoryMapping=false csvfile=h.csv
New LoadShape.i npts=8 csvfile=i.csv
moreover MemoryMapping=Yes csvfile=l.csv
! New LoadShape.j npts=8 MemoryMapping=Yes csvfile=j.csv
New Load.k bus1=b1 MemoryMapping=Yes file=k.csv
";
    let scanned = mapped_file_arguments(probe);
    let scanned: Vec<(&str, &str)> = scanned
        .iter()
        .map(|(k, f)| (k.as_str(), f.as_str()))
        .collect();
    assert_eq!(
        scanned,
        [
            ("csvfile", "a.csv"),
            ("file", "b.txt"),
            ("pqcsvfile", "c.csv"),
            ("sngfile", "d.sng"),
            ("dblfile", "sub/e.dbl"),
            ("csvfile", "f.csv"),
        ],
        "the mapped-LoadShape scan of the probe deck"
    );

    let root = repo_root();
    let found = mapped_fixtures(&root);

    // A scan that broke or stopped seeing a fixture fails here instead of
    // greening: the expected value is the corpus's set of mapped fixtures.
    const CKT24: &str =
        "tests/corpus/electricdss-tst/Version8/Distrib/Examples/MemoryMappingLoadShapes/ckt24";
    const MMF: &str = "tests/corpus/modes/inputformat/shape_mmf";
    const MMF_IO: &str = "tests/corpus/modes/inputformat/shape_mmf_io";
    const SINGLECOL: &str = "tests/corpus/modes/upgrade/mmf_singlecol";
    let expected: BTreeSet<String> = [
        format!("{CKT24}/LS_Phase_AOK.csv"),
        format!("{CKT24}/LS_Phase_AOK.txt"),
        format!("{CKT24}/myDBL.dbl"),
        format!("{CKT24}/mySGL.sng"),
        format!("{MMF}/mm8.dbl"),
        format!("{MMF}/mm8.sng"),
        format!("{MMF}/mmpq8.csv"),
        format!("{MMF_IO}/mm8.dbl"),
        format!("{MMF_IO}/mm8.sng"),
        format!("{MMF_IO}/mmpq8_plain.csv"),
        format!("{SINGLECOL}/mm8.csv"),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        found.keys().cloned().collect::<BTreeSet<_>>(),
        expected,
        "the corpus's memory-mapped LoadShape fixtures moved: check the new or changed \
         fixture's .gitattributes and update this expected value"
    );

    let normalized: Vec<String> = found
        .iter()
        .filter(|(file, _)| !is_checked_out_verbatim(&root, file))
        .map(|(file, fixture)| format!("{file} (read by {:?})", fixture.decks))
        .collect();
    assert!(
        normalized.is_empty(),
        "memory-mapped LoadShape fixtures that git may EOL-normalize on checkout — declare \
         each `-text` in a .gitattributes beside it:\n  {}",
        normalized.join("\n  ")
    );

    let converted: Vec<String> = found
        .iter()
        .filter(|(_, fixture)| !fixture.binary)
        .filter(|(file, _)| {
            std::fs::read(root.join(file))
                .unwrap_or_else(|e| panic!("read {file}: {e}"))
                .contains(&b'\r')
        })
        .map(|(file, fixture)| format!("{file} (read by {:?})", fixture.decks))
        .collect();
    assert!(
        converted.is_empty(),
        "memory-mapped LoadShape text fixtures holding CR bytes, where every one is committed \
         with LF records: git converted the copy before its `-text` declaration landed and does \
         not rewrite it on a later checkout or merge. Delete each file and check it out again:\n  \
         {}",
        converted.join("\n  ")
    );
}

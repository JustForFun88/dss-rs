//! The corpus gate's view of the shared per-run scratch copies
//! (RETRO_FIXES_PLAN.md RF-I00-01, `INFRA|1`): the implementation lives in
//! `harness/scratch.rs` (shared with the pin binaries that run vendored
//! decks); this module re-exports it under the gate's `crate::scratch` path
//! and keeps its rails, which need the manifest.

pub(crate) use crate::harness::scratch::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};

    fn fixture_root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "dss_scratch_{tag}_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    /// Every run gets its own fresh directory: two copies of one case for one
    /// producer never share a directory, both hold the deck, and each is gone
    /// after its removal.
    #[test]
    fn two_runs_of_one_case_get_two_fresh_copies_and_both_are_removed() {
        let deck = crate::manifest::corpus_file("Test/AutoTrans/Auto3bus.dss");
        let a = ScratchCopy::new(&deck, PORT);
        let b = ScratchCopy::new(&deck, PORT);
        assert_ne!(a.run_dir(), b.run_dir(), "a run directory is never reused");
        for c in [&a, &b] {
            assert!(
                Path::new(c.deck()).is_file(),
                "{:?} holds the deck",
                c.run_dir()
            );
            assert!(
                c.run_dir().starts_with(scratch_root()),
                "the copy lives under the gate's scratch root, never in tests/corpus"
            );
        }
        let (da, db) = (a.run_dir().to_path_buf(), b.run_dir().to_path_buf());
        a.finish();
        b.finish();
        assert!(!da.exists() && !db.exists());
    }

    /// A run directory must not pre-exist: [`ScratchCopy::new_in`], the code
    /// every copy is made by, is handed a nonce whose directory is already
    /// there and must refuse it (`create_dir` on the last level) instead of
    /// running in a stale or colliding directory that silently carries another
    /// run's files (RETRO_FIXES RF-I00-01 settlement: the rail used to exercise
    /// `std::fs::create_dir` alone, so `create_dir_all` there stayed green).
    #[test]
    fn a_run_directory_that_already_exists_is_refused() {
        let deck = crate::manifest::corpus_file("Test/AutoTrans/Auto3bus.dss");
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let nonce = format!("fresh-rail-{}-{nanos:x}", std::process::id());
        let first = ScratchCopy::new_in(&deck, PORT, nonce.clone());
        assert!(first.run_dir().ends_with(&nonce), "{:?}", first.run_dir());
        assert!(
            Path::new(first.deck()).is_file(),
            "the first copy holds the deck"
        );
        let again = std::panic::catch_unwind(|| ScratchCopy::new_in(&deck, PORT, nonce.clone()));
        let msg = match again {
            Ok(reused) => {
                reused.finish();
                first.finish();
                panic!("a pre-existing run directory was reused instead of refused");
            }
            Err(e) => crate::runner::panic_msg(e),
        };
        assert!(
            msg.contains("could not be created fresh")
                && msg.contains("Test/AutoTrans/Auto3bus.dss")
                && msg.contains("(port)"),
            "the refusal names the case and the producer: {msg}"
        );
        assert!(
            Path::new(first.deck()).is_file(),
            "the refusal left the first copy alone"
        );
        first.finish();
    }

    /// The copy-removal budget is 25 attempts 200 ms apart (RETRO_FIXES
    /// RF-I00-01 part 1), and `TESTING.md` states the same numbers. Coordinator
    /// ruling 2026-09-26 16:40: a copy that survives it is a producer to fix,
    /// never a budget to widen, so a widened budget reds here instead of only
    /// slowing the held-file rail down.
    #[test]
    fn the_copy_removal_budget_is_25_attempts_200_ms_apart() {
        assert_eq!(
            (COPY_REMOVE_ATTEMPTS, COPY_REMOVE_PAUSE),
            (25, std::time::Duration::from_millis(200)),
            "the copy-removal budget moved: fix the producer, never widen the budget"
        );
        let testing =
            std::fs::read_to_string(corpus_root().join("../../TESTING.md")).expect("TESTING.md");
        assert!(
            testing.contains("25 x 200 ms"),
            "TESTING.md must state the copy-removal budget as `25 x 200 ms`"
        );
    }

    /// The D32(2) rail on its new footing: a created file the producer still
    /// holds open the FPC/Delphi way (`FILE_SHARE_READ` only — deletion NOT
    /// shared, winnt.h) outlives the whole budget and fails the case naming
    /// the producer and the file; released afterwards, the copy goes.
    #[cfg(windows)]
    #[test]
    fn a_copy_a_producer_still_holds_fails_naming_the_producer() {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        let deck = crate::manifest::corpus_file("Test/AutoTrans/Auto3bus.dss");
        let copy = ScratchCopy::new(&deck, "capi_v0145");
        let dir = copy.run_dir().to_path_buf();
        let trace = Path::new(copy.deck()).parent().unwrap().join("STOR_s1.CSV");
        std::fs::write(&trace, b"hour,t\n").unwrap();
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&trace)
            .expect("open the created file the way an engine holds its trace");
        let err = copy.remove().expect_err("a held file blocks the removal");
        assert!(
            err.contains("`capi_v0145` producer") && err.contains("STOR_s1.CSV"),
            "the failure names the producer and the survivor: {err}"
        );
        drop(held);
        remove_run_dir(&dir).expect("released, the copy goes");
    }

    /// A holder that lets go INSIDE the budget (a scan of a just-closed file,
    /// a worker exiting after its reply) is waited out, not reported.
    #[cfg(windows)]
    #[test]
    fn a_holder_released_within_the_budget_is_waited_out() {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        let root = fixture_root("transient");
        let dir = root.join("run");
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("EXP_Y.CSV"), b"y\n").unwrap();
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(dir.join("EXP_Y.CSV"))
            .unwrap();
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(COPY_REMOVE_PAUSE * 3);
            drop(held);
        });
        let res = remove_run_dir(&dir);
        releaser.join().unwrap();
        assert_eq!(
            res,
            Ok(()),
            "released inside the budget, nothing is reported"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A deck whose `../` references leave its folder carries them into the
    /// copy at the same relative place, and the copy compiles like the tree
    /// (no port error): `IEEE13Nodeckt.dss` redirects `../IEEELineCodes.DSS`.
    #[test]
    fn a_parent_reference_resolves_inside_the_copy() {
        let deck =
            crate::manifest::corpus_file("Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss");
        let c = cached_closure(&deck);
        assert!(
            fold(&c.root).ends_with("/version8/distrib/ieeetestcases"),
            "closure root {:?}",
            c.root
        );
        in_copy(&deck, PORT, |copied| {
            let codes = Path::new(copied)
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("IEEELineCodes.DSS");
            assert!(codes.is_file(), "{codes:?} is carried into the copy");
            let mut dss = dss_core::exec::Dss::new();
            dss.command(&format!("compile \"{copied}\""));
            assert!(
                dss.errors().is_empty() && dss.circuit().is_some(),
                "the copy compiles clean: {:?}",
                dss.errors()
            );
        });
    }

    /// A RELATIVE reference that climbs out of `tests/corpus/` cannot be
    /// carried into a copy, so the closure lists it like an absolute one
    /// instead of skipping it in silence (RETRO_FIXES RF-I00-01 settlement). A
    /// deck outside the corpus stands in for such a deck, since no vendored
    /// deck has one (`the_external_closures_are_the_pinned_population`).
    #[test]
    fn a_relative_reference_that_leaves_the_corpus_is_listed() {
        let root = fixture_root("escape");
        std::fs::create_dir_all(root.join("case")).unwrap();
        std::fs::write(root.join("shared.dss"), b"! shared").unwrap();
        std::fs::write(
            root.join("case/deck.dss"),
            b"redirect ../shared.dss\nnew circuit.c\n",
        )
        .unwrap();
        let c = closure_of(&root.join("case/deck.dss"));
        assert!(
            c.outside.contains("../shared.dss"),
            "a relative escape must be listed: {:?}",
            c.outside
        );
        std::fs::remove_dir_all(&root).ok();
    }

    /// `dss_epri::smoke::Ieee13Copy` spells the IEEE13 closure by hand (that
    /// crate cannot see this harness): it must carry exactly the files a
    /// [`ScratchCopy`] of the same deck carries, so the two cannot drift apart.
    #[cfg(windows)]
    #[test]
    fn the_dss_epri_ieee13_copy_carries_the_computed_closure() {
        fn files(root: &Path) -> BTreeSet<String> {
            let mut out = BTreeSet::new();
            let mut stack = vec![root.to_path_buf()];
            while let Some(dir) = stack.pop() {
                for e in std::fs::read_dir(&dir).unwrap().flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else {
                        out.insert(fold(&rel(&p, root)));
                    }
                }
            }
            out
        }
        let deck =
            crate::manifest::corpus_file("Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss");
        let ours = ScratchCopy::new(&deck, "r4133");
        let theirs = dss_epri::smoke::Ieee13Copy::new().unwrap_or_else(|e| panic!("{e}"));
        let root_of = |d: &str| {
            Path::new(d)
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .to_path_buf()
        };
        let (a, b) = (files(&root_of(ours.deck())), files(&root_of(theirs.deck())));
        ours.finish();
        theirs.remove().unwrap_or_else(|e| panic!("{e}"));
        assert!(a.len() > 1, "the computed closure carries files: {a:?}");
        assert_eq!(
            a, b,
            "Ieee13Copy's hand-spelled closure drifted from closure_of"
        );
    }

    /// The closures that reach outside their case folder, pinned by count and
    /// by their largest root: a new deck that climbs further (or a scanner
    /// change that stops seeing a reference) moves this population and must be
    /// looked at, not absorbed.
    #[test]
    fn the_external_closures_are_the_pinned_population() {
        let mut decks: Vec<String> = crate::manifest::load_solvable()
            .into_iter()
            .map(|c| crate::manifest::corpus_file(&c.path))
            .collect();
        for fam in crate::manifest::FAMILIES {
            for c in crate::manifest::load_family(fam.name) {
                decks.push(crate::manifest::family_file(fam.name, &c.path));
            }
        }
        let corpus = corpus_root();
        let mut roots: BTreeMap<String, usize> = BTreeMap::new();
        let mut outside: BTreeSet<String> = BTreeSet::new();
        for d in &decks {
            let c = cached_closure(d);
            outside.extend(c.outside.iter().cloned());
            if !c.flat.is_empty() || !c.files.is_empty() {
                *roots
                    .entry(fold(rel(&c.root, &corpus).as_path()))
                    .or_default() += 1;
            }
        }
        assert_eq!(decks.len(), 526, "the manifest population");
        assert_eq!(
            roots
                .iter()
                .map(|(r, n)| (r.as_str(), *n))
                .collect::<Vec<_>>(),
            EXTERNAL_CLOSURE_ROOTS,
            "decks whose closure leaves their folder, per closure root"
        );
        assert_eq!(
            outside.into_iter().collect::<Vec<_>>(),
            OUTSIDE_REFERENCES,
            "references that leave tests/corpus/ (kept verbatim in the copy)"
        );
    }

    /// Measured 2026-09-26 on `e5f48d53` (RF-I00-01 part 1, `part_1.md`).
    const EXTERNAL_CLOSURE_ROOTS: &[(&str, usize)] = &[
        ("electricdss-tst", 1),
        ("electricdss-tst/version8/distrib", 39),
        (
            "electricdss-tst/version8/distrib/examples/invertermodels/pvsystem/invcontrol",
            11,
        ),
        ("electricdss-tst/version8/distrib/ieeetestcases", 14),
    ];
    const OUTSIDE_REFERENCES: &[&str] = &["C:\\Users\\prdu001\\OpenDSS\\Source\\DESS1\\Dess1.DLL"];
    /// RF-I00-01 part 2 — the read-only-tree rail's photograph cannot be
    /// vacuous: an untouched tree photographs equal, and every way a producer
    /// can touch a tree shows up — a file created and swept again (its
    /// folder's mtime), a file rewritten with its own bytes (its mtime), a
    /// dropping left behind (`+`), a vendored file removed (`-`).
    #[test]
    fn a_tree_photograph_sees_every_way_a_producer_touches_the_tree() {
        let root = fixture_root("photo");
        std::fs::create_dir_all(root.join("case/sub")).unwrap();
        std::fs::write(root.join("case/deck.dss"), b"! deck").unwrap();
        std::fs::write(root.join("case/sub/keep.txt"), b"vendored").unwrap();
        // NTFS stamps times from a coarse (~16 ms) clock: step past the tick
        // the fixture was written in, or a rewrite could keep its old mtime.
        std::thread::sleep(std::time::Duration::from_millis(100));
        let before = TreePhoto::take(&root);
        assert_eq!(before.len(), 5, "root + case + sub + two files");
        assert_eq!(
            before.changes(&TreePhoto::take(&root)),
            Vec::<String>::new()
        );

        // Created and swept again: nothing left, but the folder moved.
        std::fs::write(root.join("case/sub/export.csv"), b"run output").unwrap();
        std::fs::remove_file(root.join("case/sub/export.csv")).unwrap();
        assert_eq!(before.changes(&TreePhoto::take(&root)), ["~case/sub"]);

        // Rewritten with its own bytes: same length, new mtime.
        std::fs::write(root.join("case/deck.dss"), b"! deck").unwrap();
        // A dropping left behind, and a vendored file gone.
        std::fs::write(root.join("case/left.csv"), b"dropping").unwrap();
        std::fs::remove_file(root.join("case/sub/keep.txt")).unwrap();
        assert_eq!(
            before.changes(&TreePhoto::take(&root)),
            [
                "~case",
                "~case/deck.dss",
                "+case/left.csv",
                "~case/sub",
                "-case/sub/keep.txt"
            ]
        );
        std::fs::remove_dir_all(&root).ok();
    }

    /// RF-I00-01 part 2 — the gate-side entry points refuse the vendored tree
    /// in every spelling (case, the platform separator, `..`) and pass a
    /// scratch copy or a fixture outside it.
    #[test]
    fn a_vendored_path_is_refused_and_a_copy_passes() {
        let refused = |p: PathBuf| {
            let caught = std::panic::catch_unwind(|| {
                not_vendored(&p, "fixture");
            });
            let msg =
                crate::runner::panic_msg(caught.expect_err("a vendored path must be refused"));
            assert!(
                msg.contains("lies inside the vendored corpus") && msg.contains("fixture"),
                "{msg}"
            );
        };
        let corpus = corpus_root();
        refused(corpus.join("electricdss-tst/Test/AutoTrans/Auto1bus.dss"));
        let sep = std::path::MAIN_SEPARATOR.to_string();
        refused(PathBuf::from(
            format!("{}/ELECTRICDSS-TST/test/Auto.dss", corpus.display())
                .to_uppercase()
                .replace('/', &sep),
        ));
        refused(corpus.join("modes/../electricdss-tst/Test"));
        refused(corpus.clone());
        let copy = scratch_root().join("0123456789abcdef/port/run/Test/deck.dss");
        assert_eq!(not_vendored(&copy, "fixture"), copy.as_path());
        let fixture = fixture_root("not_vendored");
        assert_eq!(not_vendored(&fixture, "fixture"), fixture.as_path());
        // A sibling whose name merely STARTS like the corpus root is outside it.
        let sibling = PathBuf::from(format!("{}-sibling/deck.dss", corpus.display()));
        assert_eq!(not_vendored(&sibling, "fixture"), sibling.as_path());
        std::fs::remove_dir_all(&fixture).ok();
    }
}

//! Corpus-hygiene guard — a Rust port of `tools/oracle/corpus_guard.py` (and the
//! identical `CorpusGuard` in `crates/dss-core/tests/corpus_live.rs`). A deck's
//! `Compile` sets `OutputDirectory := <case dir>`, so a migrated deck's
//! `Export`/`Show`/`Save` (and any `<CircuitName>/DI_yr_*` demand-interval tree)
//! writes report files next to the vendored deck. This snapshots the case dir
//! before the run and restores it on drop: delete anything the run created,
//! rewrite any small pre-existing file it overwrote.
//!
//! The snapshot is RECURSIVE, and — critically — an *incomplete* snapshot
//! (any `read_dir` failure) disables deletion entirely, so a transient lock can
//! never make the guard classify pre-existing corpus files as run-created and
//! delete them (the war story documented in the Python original).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Buffer files up to this size for overwrite-restore (OpenDSS writes small text
/// reports, never the multi-MiB input data files). Mirrors the Python
/// `_RESTORE_MAX` and the Rust gate's `RESTORE_MAX`.
const RESTORE_MAX: u64 = 2 * 1024 * 1024;

pub struct CorpusGuard {
    dir: PathBuf,
    names: BTreeSet<String>,
    buf: BTreeMap<String, Vec<u8>>,
    snapshot_ok: bool,
}

impl CorpusGuard {
    pub fn new(case_path: &str) -> CorpusGuard {
        let dir = Path::new(case_path)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let mut names = BTreeSet::new();
        let mut buf = BTreeMap::new();
        let snapshot_ok = Self::snapshot(&dir, "", &mut names, &mut buf);
        CorpusGuard {
            dir,
            names,
            buf,
            snapshot_ok,
        }
    }

    /// Recursively record every entry (files AND directories) as `/`-joined
    /// relative paths, buffering small files. Returns false if any directory
    /// listing failed (caller disables deletion).
    fn snapshot(
        dir: &Path,
        prefix: &str,
        names: &mut BTreeSet<String>,
        buf: &mut BTreeMap<String, Vec<u8>>,
    ) -> bool {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return false;
        };
        let mut ok = true;
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            names.insert(rel.clone());
            if is_dir {
                ok &= Self::snapshot(&entry.path(), &rel, names, buf);
                continue;
            }
            let small = entry
                .metadata()
                .map(|m| m.is_file() && m.len() <= RESTORE_MAX)
                .unwrap_or(false);
            if small && let Ok(data) = std::fs::read(entry.path()) {
                buf.insert(rel, data);
            }
        }
        ok
    }

    fn sweep_created(&self, dir: &Path, prefix: &str) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if self.names.contains(&rel) {
                if is_dir {
                    self.sweep_created(&entry.path(), &rel);
                }
                continue;
            }
            if is_dir {
                let _ = std::fs::remove_dir_all(entry.path());
            } else {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

impl Drop for CorpusGuard {
    fn drop(&mut self) {
        if !self.snapshot_ok {
            return;
        }
        let dir = self.dir.clone();
        self.sweep_created(&dir, "");
        for (name, data) in &self.buf {
            let p = self.dir.join(name);
            match std::fs::read(&p) {
                Ok(cur) if cur == *data => {}
                _ => {
                    let _ = std::fs::write(&p, data);
                }
            }
        }
    }
}

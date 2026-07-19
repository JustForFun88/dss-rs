"""Corpus-hygiene guard used by the oracle server (`oracle_server.py`).

Lifted move-only from oracle_server.py into its own module (a Rust port,
`crates/dss-epri/src/guard.rs`, guards the r4133 bridge) — see the class
docstring for the snapshot-failure war story. The snapshot is RECURSIVE (WP8.8):
run-created
files inside pre-existing subdirectories and run-created directory trees (the
`<CircuitName>/DI_yr_*` demand-interval tree) are both detected and removed.
Writes OUTSIDE the case-dir tree (e.g. a manual `dss-cli` run from elsewhere)
remain uncoverable here — sweep workflows must still end with a
`git status tests/corpus` check (recovery: `git restore tests/corpus` /
`git clean`).
"""

from __future__ import annotations

import os
import shutil

_RESTORE_MAX = 2 * 1024 * 1024  # buffer files up to 2 MiB for overwrite-restore


class CorpusGuard:
    """Restore the case's directory after a run: delete any file the run
    created, and rewrite any small pre-existing file it overwrote. Large files
    (> `_RESTORE_MAX`) are not buffered — OpenDSS only writes small text reports,
    never the multi-MiB data files (loadshape CSVs, etc.).

    `_snapshot_ok` mirrors the Rust `CorpusGuard` (corpus_gate.rs): if the
    pre-run snapshot fails or is cut short, `__exit__` must not delete anything —
    a truncated `names` set would classify pre-existing corpus files as
    run-created and delete them (empirically demonstrated: a transient lock on
    one file mid-snapshot used to abort the whole listing via the old
    whole-loop `except OSError`, and the exit pass then deleted every corpus
    file that sorted after it, `YgD-Test.dss` included). Per-file failures now
    only skip that file's overwrite-restore buffer; a failed DIRECTORY listing
    anywhere in the recursive walk disables deletion entirely (incomplete
    snapshot = never delete)."""

    def __init__(self, case_path: str):
        self.dir = os.path.dirname(os.path.abspath(case_path))
        # `/`-joined paths relative to `self.dir`, files AND directories.
        self.names: set[str] = set()
        self.buf: dict[str, bytes] = {}
        self._snapshot_ok = False

    def _snapshot(self, d: str, prefix: str) -> bool:
        """Recursively track every entry under `d`. Returns False if any
        directory listing failed (caller disables deletion). Never follows
        symlinks/junctions (none are expected in the corpus)."""
        try:
            listing = os.listdir(d)
        except OSError:
            return False
        ok = True
        for name in listing:
            p = os.path.join(d, name)
            rel = f"{prefix}/{name}" if prefix else name
            self.names.add(rel)
            try:
                if os.path.isdir(p) and not os.path.islink(p):
                    ok = self._snapshot(p, rel) and ok
                    continue
                if os.path.isfile(p) and os.path.getsize(p) <= _RESTORE_MAX:
                    with open(p, "rb") as fh:
                        self.buf[rel] = fh.read()
            except OSError:
                # Unreadable (e.g. transiently locked): it is still a
                # pre-existing entry — it stays in `names` so it is never
                # deleted; only its overwrite-restore is unavailable.
                pass
        return ok

    def __enter__(self) -> "CorpusGuard":
        self._snapshot_ok = self._snapshot(self.dir, "")
        return self

    def _sweep_created(self, d: str, prefix: str) -> None:
        """Delete entries whose relative path is absent from the pre-run
        snapshot. A run-created directory is removed wholesale; a pre-existing
        one is recursed to find run-created files inside it."""
        try:
            current = os.listdir(d)
        except OSError:
            return
        for name in current:
            p = os.path.join(d, name)
            rel = f"{prefix}/{name}" if prefix else name
            is_dir = os.path.isdir(p) and not os.path.islink(p)
            if rel in self.names:
                if is_dir:
                    self._sweep_created(p, rel)  # pre-existing fixture subdir
                continue
            try:
                if is_dir:
                    # Run-created directory (the DI `<CircuitName>/` tree).
                    # The engines never create junctions/links here, and only
                    # paths absent from the pre-run snapshot are removed.
                    shutil.rmtree(p, ignore_errors=True)
                else:
                    os.remove(p)
            except OSError:
                pass

    def __exit__(self, *exc) -> bool:
        if not self._snapshot_ok:
            return False
        self._sweep_created(self.dir, "")
        for rel, data in self.buf.items():  # overwritten by the run
            p = os.path.join(self.dir, rel)
            try:
                with open(p, "rb") as fh:
                    if fh.read() == data:
                        continue
                with open(p, "wb") as fh:
                    fh.write(data)
            except OSError:
                pass
        return False

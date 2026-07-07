"""Corpus-hygiene guard shared by the oracle server and the DSS-Python
validation harness (tools/opendss/dsspy_validation/).

Lifted move-only from oracle_server.py so both consumers use the identical,
empirically-hardened implementation (see the class docstring for the
snapshot-failure war story). NOT recursive: files created in *sub*directories
of the case dir escape it — sweep workflows must end with a
`git status tests/corpus` check (recovery: `git restore tests/corpus`).
"""

from __future__ import annotations

import os

_RESTORE_MAX = 2 * 1024 * 1024  # buffer files up to 2 MiB for overwrite-restore


class CorpusGuard:
    """Restore the case's directory after a run: delete any file the run
    created, and rewrite any small pre-existing file it overwrote. Large files
    (> `_RESTORE_MAX`) are not buffered — OpenDSS only writes small text reports,
    never the multi-MiB data files (loadshape CSVs, etc.).

    `_snapshot_ok` mirrors the Rust `CorpusGuard` (corpus_live.rs): if the
    pre-run snapshot fails or is cut short, `__exit__` must not delete anything —
    a truncated `names` set would classify pre-existing corpus files as
    run-created and delete them (empirically demonstrated: a transient lock on
    one file mid-snapshot used to abort the whole listing via the old
    whole-loop `except OSError`, and the exit pass then deleted every corpus
    file that sorted after it, `YgD-Test.dss` included). Per-file failures now
    only skip that file's overwrite-restore buffer."""

    def __init__(self, case_path: str):
        self.dir = os.path.dirname(os.path.abspath(case_path))
        self.names: set[str] = set()
        self.buf: dict[str, bytes] = {}
        self._snapshot_ok = False

    def __enter__(self) -> "CorpusGuard":
        try:
            listing = os.listdir(self.dir)
        except OSError:
            return self  # snapshot failed -> deletion stays disabled
        for name in listing:
            p = os.path.join(self.dir, name)
            try:
                if not os.path.isfile(p):
                    continue
                self.names.add(name)
                if os.path.getsize(p) <= _RESTORE_MAX:
                    with open(p, "rb") as fh:
                        self.buf[name] = fh.read()
            except OSError:
                # Unreadable (e.g. transiently locked): it is still a
                # pre-existing file — keep it in `names` so it is never
                # deleted; only its overwrite-restore is unavailable.
                self.names.add(name)
        self._snapshot_ok = True
        return self

    def __exit__(self, *exc) -> bool:
        if not self._snapshot_ok:
            return False
        try:
            current = set(os.listdir(self.dir))
        except OSError:
            return False
        for name in current - self.names:  # created by the run
            try:
                os.remove(os.path.join(self.dir, name))
            except OSError:
                pass
        for name, data in self.buf.items():  # overwritten by the run
            p = os.path.join(self.dir, name)
            try:
                with open(p, "rb") as fh:
                    if fh.read() == data:
                        continue
                with open(p, "wb") as fh:
                    fh.write(data)
            except OSError:
                pass
        return False

"""Corpus-hygiene guard used by the oracle server (`oracle_server.py`).

Lifted move-only from oracle_server.py into its own module (a Rust port,
`crates/dss-epri/src/guard.rs`, guards the r4133 bridge) — see the class
docstring for the snapshot-failure war story. The snapshot is RECURSIVE (WP8.8),
and since GOLDEN_REBASE G1.10a (decision D30(2)) the CLASSIFICATION covers the
case dir's own entries plus everything under a directory the run created (the
`<CircuitName>/DI_yr_*` demand-interval tree, removed as one tree). A
PRE-EXISTING subdirectory is deliberately NOT descended into — nothing this run
writes can land there, and what does land there belongs to a concurrently
running sibling case (citations + measurement:
`crates/dss-epri/src/guard.rs::CorpusGuard::classify`).
Writes OUTSIDE the case-dir tree (e.g. a manual `dss-cli` run from elsewhere)
remain uncoverable here — sweep workflows must still end with a
`git status tests/corpus` check (recovery: `git restore tests/corpus` /
`git clean`).

GOLDEN_REBASE G1.10a added the *reporting* half: the same classification that
decides what to delete is also the gate's `compare_run_files` surface (the set
of filesystem entries a run created under the case dir). One classification,
two consumers — see `CorpusGuard.created`. And since decision D32(2) the sweep
is no longer allowed to fail quietly: whatever it could not remove is left in
`CorpusGuard.sweep_failed` for the caller to report (`oracle_server.py` puts it
in the reply, and the gate's runner fails the case), so a producer that leaks an
open file handle can never again silence a LATER producer's created set by
leaving the file behind as "pre-existing". Run
`python corpus_guard.py --self-test` for the shared synthetic fixture the Rust
twin asserts as well — including the leak fixture.
"""

from __future__ import annotations

import os
import shutil

_RESTORE_MAX = 2 * 1024 * 1024  # buffer files up to 2 MiB for overwrite-restore


# --- GOLDEN_REBASE G1.10a: the created-file SET ------------------------------
#
# Three producers report this set — this guard (capi channel), its Rust twin
# `crates/dss-epri/src/guard.rs` (r4133 channel) and the port-side probe in the
# corpus gate — and the comparator compares them as an exact discrete set at
# `rel = abs = 0`. They must therefore agree on ONE spelling of a name, so the
# canonical form is defined here and mirrored byte-for-byte in Rust
# (`normalize_created_name`).
#
# The case fold is a CROSS-ORACLE NORMALIZATION, not a tolerance: r4133 writes
# 'EXP_VOLTAGES.CSV' (`Version8/Source/Executive/ExportOptions.pas:333-356`)
# while dss_capi 0.14.5 writes 'EXP_VOLTAGES.csv'
# (`src/Executive/ExportOptions.pas:314,343,345,381,437`), and r4133
# additionally lowercases deck-supplied stems
# ('Auto1bus_HL_current.txt' -> 'auto1bus_hl_current.txt'), so no single
# spelling can satisfy both gating channels. NTFS is case-insensitive, so
# nothing observable depends on the case (the R-18 decision, recorded in
# DIVERGENCES.md).
#
# Provenance: the upstream harness never compared this set at all — fastdss's
# `tests/compare_outputs.py` walks the reference zip file list and silently
# skips a name missing on the other side (`except KeyError: ... continue`,
# :416-421), and its CSV comparison prints instead of failing
# (`except: print("COMPARE CSV ERROR:", fn)`, :517-524). New coverage, not
# catch-up.


# Engine-internal scratch files. The Pascal engines round-trip the fundamental
# solution through DISK when harmonics is initialized: `SavePresentVoltages`
# writes '<CircuitName_>SavedVoltages.dbl' (r4133
# `Version8/Source/Common/Utilities.pas:1512-1521`, reached only from
# `InitializeForHarmonics` :1599-1608; read back by `RetrieveSavedVoltages`
# :1554-1564, consumed at `Common/SolutionAlgs.pas:1056,1131`; dss_capi 0.14.5
# twin `src/Common/Utilities.pas:883` / :914-923, consumed at
# `src/Common/SolutionAlgs.pas:1030,1110`). The port keeps that vector in memory
# (`crates/dss-core/src/solution/solution/state.rs:329-331`,
# `solution/solution/harmonics.rs:222`) and writes no such file, so the name is
# split off SYMMETRICALLY on every channel (coordinator decision D25/Q2)
# instead of becoming one ledger row per harmonics deck.
#
# NOT scratch: '<CircuitName_>SavedVoltages.Txt', the user-visible
# `Save Voltages` output that `VDIFF` reads back (r4133
# `Common/Solution.pas:3973` + `Executive/ExecHelper.pas:3321`, capi
# `src/Common/Solution.pas:2288` + `src/Executive/ExecHelper.pas:3387`, port
# `crates/dss-core/src/exec/report.rs:2490-2522`) — all three engines write it,
# so it stays a compared member of the set.
ENGINE_SCRATCH_SUFFIXES = ("savedvoltages.dbl",)


def ascii_lower(name: str) -> str:
    """ASCII-only lowercase — the exact twin of Rust `str::to_ascii_lowercase`.

    `str.lower()` is Unicode-aware and would fold code points that Rust leaves
    alone, which would make the two guards disagree on a name no corpus deck
    produces today. Writing the fold out keeps them identical."""
    return "".join(chr(ord(c) + 32) if "A" <= c <= "Z" else c for c in name)


def normalize_created_name(rel: str, is_dir: bool) -> str:
    """Canonical member of the created-file set: `/`-joined, `./`-stripped,
    ASCII-case-folded, with a trailing `/` marking a run-created DIRECTORY (its
    contents are separate members). Idempotent, so a consumer may re-apply
    it."""
    s = rel.replace("\\", "/")
    while s.startswith("./"):
        s = s[2:]
    while "//" in s:
        s = s.replace("//", "/")
    s = ascii_lower(s.rstrip("/"))
    return f"{s}/" if is_dir else s


def is_engine_scratch_file(name: str) -> bool:
    """True for an engine-internal scratch file (see `ENGINE_SCRATCH_SUFFIXES`):
    one the Pascal engines write only to read back within the same run."""
    folded = ascii_lower(name)
    return any(folded.endswith(sfx) for sfx in ENGINE_SCRATCH_SUFFIXES)


def split_engine_scratch(names) -> tuple[list[str], list[str]]:
    """Partition a created-file set into (compared, engine-internal scratch).

    The structural normalization of decision D25/Q2, applied SYMMETRICALLY to
    every channel: the caller compares the first list and counts the second (the
    gate's fail-on-stale `SCRATCH_FILE_DECLINES` population constant), so the
    decline stays visible instead of being silently masked."""
    kept: list[str] = []
    scratch: list[str] = []
    for n in names:
        (scratch if is_engine_scratch_file(n) else kept).append(n)
    return kept, scratch


# ---------------------------------------------------------------------------
# GOLDEN_REBASE G1.10b - which created files carry their CONTENTS to the gate.
# ---------------------------------------------------------------------------


def check_contents_pattern(pattern: str) -> None:
    """Refuse a selection pattern that is not exactly one `*` between two
    literal ASCII parts.

    The shape is pinned rather than grown into a glob dialect because the same
    pattern list is matched on the Rust side
    (`crates/dss-epri/src/guard.rs::check_contents_pattern`): two glob engines
    that disagreed on one deck would silently compare different files on the two
    gating channels."""
    if pattern.count("*") != 1:
        raise ValueError(
            f"run-file contents pattern {pattern!r} must carry exactly one `*` "
            "(it stands for one run of characters other than `/`); the shape is "
            "fixed because crates/dss-epri/src/guard.rs matches the same patterns"
        )
    if not pattern.isascii() or pattern != ascii_lower(pattern):
        raise ValueError(
            f"run-file contents pattern {pattern!r} must be lower-case ASCII: it "
            "is matched against a `normalize_created_name` member, which is "
            "already ASCII-case-folded"
        )


def contents_pattern_matches(pattern: str, name: str) -> bool:
    """True when `pattern` - one `*` standing for any run of characters other
    than `/` - matches the normalized created-set member `name`.

    Twin: `crates/dss-epri/src/guard.rs::contents_pattern_matches`."""
    head, _, tail = pattern.partition("*")
    if len(name) < len(head) + len(tail):
        return False
    if not (name.startswith(head) and name.endswith(tail)):
        return False
    return "/" not in name[len(head) : len(name) - len(tail)]


def selects_contents(patterns, name: str) -> bool:
    """True when the gate asked for this member's contents. A created DIRECTORY
    (trailing `/`) never matches: only file bytes travel.

    The pattern list is NOT defined here - it is declared once on the gate side
    (`crates/dss-epri/src/guard.rs::RUN_FILE_CONTENTS_PATTERNS`) and arrives in
    the run request, so no transport re-derives which files it must hand back.

    Twin: `crates/dss-epri/src/guard.rs::selects_contents`."""
    if name.endswith("/"):
        return False
    return any(contents_pattern_matches(p, name) for p in patterns)


def _resolve_selected(case_dir: str, created, patterns) -> list:
    """Resolve the selected members of `created` to the real paths they were
    written under, listing `case_dir` ONCE.

    Not a second classification: `created` is the one `CorpusGuard.created()`
    already made, and this only maps each selected member back to the spelling
    its producer used on disk (`normalize_created_name` folds the case, so a
    normalized name is not a path). Every failure raises - a selected report
    that is not a direct child of the case directory, or that the directory no
    longer lists as a file, must never turn into a quietly missing file.

    Twin: `crates/dss-epri/src/guard.rs::resolve_selected`."""
    for p in patterns:
        check_contents_pattern(p)
    wanted = [n for n in created if selects_contents(patterns, n)]
    if not wanted:
        return []
    on_disk = {}
    for entry in os.listdir(case_dir):
        p = os.path.join(case_dir, entry)
        if os.path.isfile(p):
            on_disk[normalize_created_name(entry, False)] = p
    out = []
    for name in wanted:
        if "/" in name:
            raise RuntimeError(
                f"run-file contents: the selected member {name!r} lives under a "
                "run-created subdirectory. Every selected report is written to "
                "`GetOutputDirectory + CircuitName_ + FileName` (r4133 "
                "Version8/Source/Executive/ExportOptions.pas:401), i.e. into the "
                "case directory itself; a nested one means the selection patterns "
                "now reach a tree they were not written for."
            )
        if name not in on_disk:
            raise RuntimeError(
                f"run-file contents: {name!r} is in this run's created-file set "
                f"but the case directory {case_dir} no longer lists it as a file. "
                "A selected report must still be on disk when its contents are "
                "taken - the read is the last thing the run does, inside the "
                "guard scope, before the sweep."
            )
        out.append((name, on_disk[name]))
    return out


def _refuse_sidecar_under_case_dir(case_dir: str, sidecar: str) -> None:
    """Refuse a sidecar that is, contains, or sits under the case directory.

    The sidecar is cleared with a recursive delete, so the one destructive path
    of this surface states D40(6) itself rather than trusting its caller.

    Twin: `crates/dss-epri/src/guard.rs::refuse_sidecar_under_case_dir`."""
    c = os.path.abspath(case_dir)
    d = os.path.abspath(sidecar)
    if c == d or d.startswith(c + os.sep) or c.startswith(d + os.sep):
        raise RuntimeError(
            "run-file contents: the sidecar {} is the case directory {} or "
            "shares a path with it. The bytes travel through a directory the "
            "GATE owns under `target/` and this transport clears it with a "
            "recursive delete (coordinator decision D40(6)); pointing it at a "
            "corpus deck would delete vendored sources.".format(d, c)
        )


def copy_selected_contents(case_dir: str, created, patterns, sidecar):
    """Copy the selected members' bytes into the gate-owned sidecar directory
    and return the names copied, sorted - the capi transport's half of the
    G1.10b contents surface (coordinator decision D40(6)).

    Bytes travel on disk rather than inline in the line-JSON reply because the
    selection reaches ~19 MB per channel per full drive (`NEV_EXP_Y.csv` alone
    is 4.6 MB), which would cross the worker pipes twice on every `both` case.

    The sidecar is WIPED and recreated here, so a file left by an earlier case
    or by the other channel can never be read as this run's output; the gate
    then asserts the directory holds exactly the returned names and deletes it.

    `patterns` empty -> the gate did not ask: no directory is touched and `None`
    comes back, which keeps an off-flag reply identical to a pre-G1.10b one and
    lets the gate's presence rail tell "not requested" from "requested, and this
    deck wrote none of the selected reports".

    Twin: `crates/dss-epri/src/guard.rs::copy_selected_contents`."""
    if not patterns:
        return None
    if not sidecar:
        raise RuntimeError(
            "run-file contents: the request carries selection patterns but no "
            "`run_file_contents_dir`. The bytes travel through a gate-owned "
            "sidecar directory; without one the transport would have to inline "
            "megabytes into the reply, which coordinator decision D40(6) rules out."
        )
    if created is None:
        raise RuntimeError(
            "run-file contents: the request asks for file contents but this run's "
            "created-file set could not be classified (incomplete pre-run "
            "snapshot). The contents are a subset of that set, so they cannot be "
            "reported either."
        )
    selected = _resolve_selected(case_dir, created, patterns)
    # The next statement is a RECURSIVE DELETE of a path the request chose, so
    # coordinator decision D40(6) ("the gate's OWN scratch - never inside
    # `tests/corpus/` or the case directory") is enforced here, not only at the
    # single construction site. Twin: `guard.rs::refuse_sidecar_under_case_dir`.
    _refuse_sidecar_under_case_dir(case_dir, sidecar)
    shutil.rmtree(sidecar, ignore_errors=True)
    os.makedirs(sidecar, exist_ok=True)
    names = []
    for name, path in selected:
        shutil.copyfile(path, os.path.join(sidecar, name))
        names.append(name)
    return sorted(names)


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
        # Filled by `__exit__` (G1.10a, decision D32(2)): the normalized names of
        # created entries the sweep could not remove. Read it AFTER the `with`
        # block; `None` until the scope closes, so a caller can never mistake
        # "not swept yet" for "swept clean".
        self.sweep_failed: list[str] | None = None
        self._snapshot_ok = False
        self._exited = False

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

    def _classify(self, d: str, prefix: str, roots: list, out: list) -> bool:
        """The ONE created-entry classification (G1.10a): an entry of the case
        directory ITSELF is run-created iff its `/`-joined relative path is
        absent from the pre-run snapshot. The surface is the case dir's own
        entries, plus everything under a directory the run created — a
        PRE-EXISTING subdirectory is never descended into (decision D30(2),
        measured by G1.10a micro-part F2b; the r4133/capi citations and the
        measurement live on the Rust twin
        `crates/dss-epri/src/guard.rs::CorpusGuard::classify`).

        Fills `roots` with the `(abs path, is_dir)` of every created entry —
        exactly what the sweep removes — and `out` with the normalized name of
        every created entry, recursing into a created directory so its contents
        are members of the set too.

        Returns False if any directory listing failed: an incomplete
        classification may still be SWEPT (the sweep removes only what it did
        classify, never more) but must never be REPORTED as the created set."""
        try:
            current = os.listdir(d)
        except OSError:
            return False
        ok = True
        for name in current:
            p = os.path.join(d, name)
            rel = f"{prefix}/{name}" if prefix else name
            is_dir = os.path.isdir(p) and not os.path.islink(p)
            if rel in self.names:
                # Pre-existing. A pre-existing DIRECTORY is deliberately not
                # descended into: nothing this run writes can land there, and
                # what does land there belongs to a concurrently running
                # sibling case (see this method's docstring).
                continue
            out.append(normalize_created_name(rel, is_dir))
            roots.append((p, is_dir))
            if is_dir:
                # Run-created directory (the DI `<CircuitName>/` tree): every
                # entry below it is run-created too, and the whole tree is one
                # removal root.
                ok = self._collect_tree(p, rel, out) and ok
        return ok

    def _collect_tree(self, d: str, prefix: str, out: list) -> bool:
        """List a run-created directory: every entry under it is run-created."""
        try:
            current = os.listdir(d)
        except OSError:
            return False
        ok = True
        for name in current:
            p = os.path.join(d, name)
            rel = f"{prefix}/{name}"
            is_dir = os.path.isdir(p) and not os.path.islink(p)
            out.append(normalize_created_name(rel, is_dir))
            if is_dir:
                ok = self._collect_tree(p, rel, out) and ok
        return ok

    def created(self) -> list[str] | None:
        """The run-created set, sorted and normalized — the gate's
        `compare_run_files` surface (GOLDEN_REBASE G1.10a).

        `None` means "cannot be reported honestly" (an incomplete pre-run
        snapshot, or a failed listing now); the gate's presence rail
        (`harness::capture_guard::require_capture_opt`) turns that into a failed
        case. An empty list is a legitimate answer — most decks create nothing.

        Read it as the LAST statement inside the `with CorpusGuard(...)` block:
        it must see every file the run wrote, and the guard's exit sweeps them
        away. Calling it afterwards is a protocol error and raises."""
        if self._exited:
            raise RuntimeError(
                "CorpusGuard.created() called after the guard scope closed; the "
                "created-file set must be read as the last statement inside "
                "`with CorpusGuard(...)`, before the sweep removes the files"
            )
        if not self._snapshot_ok:
            return None
        roots: list = []
        out: list = []
        if not self._classify(self.dir, "", roots, out):
            return None
        return sorted(set(out))

    def _sweep_created(self) -> list[str]:
        """Delete what `_classify` classified — the same classification the
        `created()` report is built from, so the reported set can never disagree
        with the swept set. A run-created directory is removed wholesale; a
        pre-existing one keeps its pre-existing contents.

        Returns the normalized names of created entries the case directory STILL
        lists afterwards — the `sweep_failed` report of decision D32(2). A failed
        removal used to be swallowed (`except OSError: pass`), and that is how
        the leak G1.10a F4 measured could hide: dss_capi never closes its Storage
        trace stream (`src/PCElements/Storage.pas:872`, freed only at
        `:871`/`:1199`), so this `os.remove` raised, the file survived, and the
        NEXT producer of the same case snapshotted it as pre-existing and
        reported an empty created set.

        The presence test is a fresh `os.listdir` of the case directory (every
        removal root is a direct child of it), not `os.path.exists`: it is the
        very listing the next producer's snapshot takes. A failed re-listing
        reports every root — an unprovable removal is never reported as a clean
        sweep. Twin: `crates/dss-epri/src/guard.rs::CorpusGuard::sweep_created`.
        """
        roots: list = []
        out: list = []
        # The completeness flag is deliberately ignored here: an incomplete walk
        # still removes every dropping it *did* classify (all of them absent
        # from the pre-run snapshot), which is strictly better hygiene than
        # skipping the sweep.
        self._classify(self.dir, "", roots, out)
        for p, is_dir in roots:
            try:
                if is_dir:
                    # The engines never create junctions/links here, and only
                    # paths absent from the pre-run snapshot are removed.
                    shutil.rmtree(p, ignore_errors=True)
                else:
                    os.remove(p)
            except OSError:
                pass
        if not roots:
            return []
        try:
            still = set(os.listdir(self.dir))
        except OSError:
            still = None
        return sorted(
            normalize_created_name(os.path.basename(p), is_dir)
            for p, is_dir in roots
            if still is None or os.path.basename(p) in still
        )

    def __exit__(self, *exc) -> bool:
        self._exited = True
        if not self._snapshot_ok:
            # No sweep is attempted at all ("incomplete snapshot = never
            # delete"), so nothing leaked THROUGH a sweep; `created()` already
            # returned None, which fails the case through the gate's presence
            # rail.
            self.sweep_failed = []
            return False
        self.sweep_failed = self._sweep_created()
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


# --- the shared synthetic fixture (GOLDEN_REBASE G1.10a) ---------------------
#
# `python corpus_guard.py --self-test` builds it in a temp dir, prints the
# classification as JSON and asserts the expected sets. The Rust twin
# (`crates/dss-epri/src/guard.rs`, `classifies_the_shared_synthetic_fixture`)
# builds the identical tree and asserts the identical lists, so the two guards
# are provably one classification written twice.

SELF_TEST_PRE_EXISTING = ("case.dss", "root.txt", "pre/keep.txt")
SELF_TEST_RUN_WRITES = (
    "EXP_Y.CSV",
    "pre/New_Report.Txt",
    "DI_yr_0/Totals_1.CSV",
    "DI_yr_0/Sub/deep.DBL",
    "NEV_SavedVoltages.dbl",
)
# `pre/new_report.txt` is deliberately ABSENT: it sits under the pre-existing
# `pre/`, which the classification does not descend into (D30(2)) — it stands
# for a concurrently running sibling case's live report file.
SELF_TEST_CREATED = [
    "di_yr_0/",
    "di_yr_0/sub/",
    "di_yr_0/sub/deep.dbl",
    "di_yr_0/totals_1.csv",
    "exp_y.csv",
    "nev_savedvoltages.dbl",
]
SELF_TEST_SCRATCH = ["nev_savedvoltages.dbl"]
# What the case dir holds after the sweep: the pre-existing files, plus the one
# write under the pre-existing subdirectory the guard must neither report nor
# delete.
SELF_TEST_AFTER_SWEEP = sorted(SELF_TEST_PRE_EXISTING + ("pre/New_Report.Txt",))

# The D32(2) leak fixture, shared with the Rust twin
# (`guard.rs::a_created_file_the_sweep_cannot_remove_is_reported_as_sweep_failed`):
# a run creates an ordinary report and a trace file, and a producer is still
# holding the trace file open when the guard sweeps (the capi Storage stream,
# `src/PCElements/Storage.pas:872`). Python's `open` uses the Windows
# `_SH_DENYNO` share mode, which shares read and write but NOT delete, so
# `os.remove` raises exactly as it does for the real leak.
SELF_TEST_LEAK_RUN_WRITES = ("EXP_Y.CSV", "STOR_s1.CSV")
SELF_TEST_LEAK_CREATED = ["exp_y.csv", "stor_s1.csv"]
SELF_TEST_LEAK_FAILED = ["stor_s1.csv"]
SELF_TEST_LEAK_AFTER_SWEEP = ["STOR_s1.CSV", "case.dss"]

# The G1.10b CONTENTS fixture, shared with the Rust twin
# (`guard.rs::the_sidecar_copy_and_the_in_place_read_select_the_same_files`):
# one selected report written in r4133's upper-case spelling, one unselected
# text report next to it, and the selection patterns as they arrive in the run
# request. The copy carries BYTES (CRLF included) - the decode happens once, on
# the gate side (`guard.rs::decode_run_file`).
# The near misses are part of the fixture on purpose (G1.10b audit settlement,
# finding AT3-1): `*_exp_y.csv` must not take `NEV_EXP_YNodeList.csv` (the `*`
# ends at a SUFFIX, not a substring), the `*` must not span `/` (a member of a
# run-created tree), and a created DIRECTORY (trailing `/`) is never selected.
# Mutating either matcher in either language moves the COPIED list and reds -
# here and in the Rust twin, which runs this very table through its own matcher.
SELF_TEST_CONTENTS_PATTERNS = ("*_exp_y.csv", "stor_*.csv")
SELF_TEST_CONTENTS_WRITES = {
    "NEV_EXP_Y.CSV": b"Row,Col\r\n1,2\r\n",
    "NEV_VLN_Node.txt": b"not selected\n",
    "NEV_EXP_YNodeList.csv": b"not selected either\n",
}
SELF_TEST_CONTENTS_CREATED = [
    "nev_exp_y.csv",
    "nev_vln_node.txt",
    "nev_exp_ynodelist.csv",
    "ckt7/nev_exp_y.csv",
    "stor_tree/",
]
SELF_TEST_CONTENTS_COPIED = ["nev_exp_y.csv"]


def _write(path: str, text: str) -> None:
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="ascii") as fh:
        fh.write(text)


def _self_test() -> int:
    import json
    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        for rel in SELF_TEST_PRE_EXISTING:
            _write(os.path.join(tmp, rel.replace("/", os.sep)), "pre-existing\n")

        with CorpusGuard(os.path.join(tmp, "case.dss")) as g:
            for rel in SELF_TEST_RUN_WRITES:
                _write(os.path.join(tmp, rel.replace("/", os.sep)), "run-written\n")
            # the run also OVERWRITES a pre-existing file
            _write(os.path.join(tmp, "root.txt"), "clobbered\n")
            created = g.created()
            kept, scratch = split_engine_scratch(created or [])

        after = []
        for d, _dirs, files in os.walk(tmp):
            rel_d = os.path.relpath(d, tmp).replace(os.sep, "/")
            for f in files:
                after.append(f if rel_d == "." else f"{rel_d}/{f}")
        after.sort()
        with open(os.path.join(tmp, "root.txt"), encoding="ascii") as fh:
            restored = fh.read()

        try:
            g.created()
            after_scope = "NO RAISE"
        except RuntimeError:
            after_scope = "raises"
        sweep_failed = g.sweep_failed

    # D32(2): the leak fixture — a created file still held open when the guard
    # sweeps is REPORTED, never swallowed.
    with tempfile.TemporaryDirectory() as tmp2:
        _write(os.path.join(tmp2, "case.dss"), "pre-existing\n")
        with CorpusGuard(os.path.join(tmp2, "case.dss")) as g2:
            for rel in SELF_TEST_LEAK_RUN_WRITES:
                _write(os.path.join(tmp2, rel), "run-written\n")
            leak_created = g2.created()
            held = open(os.path.join(tmp2, "STOR_s1.CSV"), "rb")
        leak_failed = g2.sweep_failed
        leak_after = sorted(os.listdir(tmp2))
        held.close()

    # G1.10b: the CONTENTS sidecar copy - same selection, the same wipe-first
    # rule and the same loud failures as the Rust twin.
    with tempfile.TemporaryDirectory() as tmp3:
        _write(os.path.join(tmp3, "case.dss"), "pre-existing\n")
        for name, data in SELF_TEST_CONTENTS_WRITES.items():
            with open(os.path.join(tmp3, name), "wb") as fh:
                fh.write(data)
        # The gate's own scratch is a SIBLING of the case directory, never a
        # child: `copy_selected_contents` refuses a sidecar that shares a path
        # with the case (D40(6)).
        side = tmp3 + "-sidecar"
        os.makedirs(side, exist_ok=True)
        with open(os.path.join(side, "stale_exp_y.csv"), "wb") as fh:
            fh.write(b"from the previous case\n")
        contents_copied = copy_selected_contents(
            tmp3, SELF_TEST_CONTENTS_CREATED, SELF_TEST_CONTENTS_PATTERNS, side
        )
        contents_sidecar = sorted(os.listdir(side))
        with open(os.path.join(side, "nev_exp_y.csv"), "rb") as fh:
            contents_bytes = fh.read()
        contents_off = copy_selected_contents(tmp3, SELF_TEST_CONTENTS_CREATED, (), side)
        try:
            copy_selected_contents(
                tmp3, SELF_TEST_CONTENTS_CREATED, SELF_TEST_CONTENTS_PATTERNS, ""
            )
            contents_no_dir = "NO RAISE"
        except RuntimeError:
            contents_no_dir = "raises"
        try:
            copy_selected_contents(
                tmp3, ["gone_exp_y.csv"], SELF_TEST_CONTENTS_PATTERNS, side
            )
            contents_gone = "NO RAISE"
        except RuntimeError:
            contents_gone = "raises"
        try:
            check_contents_pattern("exp_*_*.csv")
            contents_bad_pattern = "NO RAISE"
        except ValueError:
            contents_bad_pattern = "raises"

    print(
        json.dumps(
            {
                "created": created,
                "kept": kept,
                "scratch": scratch,
                "after_sweep": after,
                "restored": restored,
                "created_after_scope": after_scope,
                "sweep_failed": sweep_failed,
                "leak_created": leak_created,
                "leak_sweep_failed": leak_failed,
                "leak_after_sweep": leak_after,
                "contents_copied": contents_copied,
                "contents_sidecar": contents_sidecar,
            },
            indent=1,
        )
    )
    assert created == SELF_TEST_CREATED, created
    assert scratch == SELF_TEST_SCRATCH, scratch
    assert kept == [n for n in SELF_TEST_CREATED if n not in SELF_TEST_SCRATCH], kept
    assert after == SELF_TEST_AFTER_SWEEP, after
    assert sweep_failed == [], sweep_failed
    assert leak_created == SELF_TEST_LEAK_CREATED, leak_created
    assert leak_failed == SELF_TEST_LEAK_FAILED, leak_failed
    assert leak_after == SELF_TEST_LEAK_AFTER_SWEEP, leak_after
    assert contents_copied == SELF_TEST_CONTENTS_COPIED, contents_copied
    assert contents_sidecar == SELF_TEST_CONTENTS_COPIED, contents_sidecar
    assert contents_bytes == SELF_TEST_CONTENTS_WRITES["NEV_EXP_Y.CSV"], contents_bytes
    assert contents_off is None, contents_off
    assert contents_no_dir == "raises", contents_no_dir
    assert contents_gone == "raises", contents_gone
    assert contents_bad_pattern == "raises", contents_bad_pattern
    assert restored == "pre-existing\n", restored
    assert after_scope == "raises", after_scope
    assert normalize_created_name("./A\\B//C.CSV", False) == "a/b/c.csv"
    assert normalize_created_name("di_yr_0/", True) == "di_yr_0/"  # idempotent
    print("OK corpus_guard self-test")
    return 0


if __name__ == "__main__":
    import sys

    if sys.argv[1:] == ["--self-test"]:
        raise SystemExit(_self_test())
    raise SystemExit("usage: python corpus_guard.py --self-test")

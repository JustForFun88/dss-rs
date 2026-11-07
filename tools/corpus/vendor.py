"""Fetch the OpenDSS test suite into `tests/corpus/electricdss-tst` (gitignored).

The corpus gate runs the decks of the repository and commit that
`tests/corpus/PIN.json` names. This script downloads that commit's tarball from
GitHub once into a cache (`DSS_CORPUS_CACHE`, default `~/.cache/dss-rs`), unpacks
it into the tree and checks every file against `tests/corpus/SHA256SUMS`. A run
also repairs the tree: a file it holds beyond the list (a deck's output) is
deleted and a changed one is written again. The marker `.dss-rs-pin` in the tree
names the commit it holds.

Text files get LF line ends, as git stores a file under `core.autocrlf`: a file
with no NUL byte and no lone CR is text. The memory-mapped LoadShape fixtures
need it, since the reader indexes their records by a fixed line width
(`crates/dss-core/tests/corpus_hygiene.rs`).

    python tools/corpus/vendor.py            # fetch, or verify and repair
    python tools/corpus/vendor.py --rehash   # after a pin change: rewrite SHA256SUMS
"""

from __future__ import annotations

import hashlib
import io
import json
import os
import re
import sys
import tarfile
import time
import urllib.request
from pathlib import Path, PurePosixPath

REPO_ROOT = Path(__file__).resolve().parents[2]
CORPUS_DIR = REPO_ROOT / "tests" / "corpus"
DST = CORPUS_DIR / "electricdss-tst"
PIN = CORPUS_DIR / "PIN.json"
SUMS = CORPUS_DIR / "SHA256SUMS"
SUMS_PREFIX = "electricdss-tst/"
MARKER = ".dss-rs-pin"
READ_TIMEOUT_S = 60
DOWNLOAD_DEADLINE_S = 1800
LONE_CR = re.compile(rb"\r(?!\n)")
COMMIT = re.compile(r"[0-9a-f]{40}")


def fail(msg: str) -> None:
    sys.exit(f"vendor.py: {msg}")


def load_pin() -> tuple[str, str]:
    """The pinned `(owner/name, commit)`."""
    try:
        pin = json.loads(PIN.read_text(encoding="utf-8"))
        repo, commit = pin["repo"], pin["commit"]
    except (OSError, ValueError, KeyError, TypeError) as e:
        fail(f"cannot read the pin {PIN}: {e}")
    if not COMMIT.fullmatch(commit):
        fail(f"{PIN}: commit {commit!r} is not a full 40-digit SHA-1")
    return repo, commit


def cache_dir() -> Path:
    env = os.environ.get("DSS_CORPUS_CACHE")
    return Path(env) if env else Path.home() / ".cache" / "dss-rs"


def tarball(repo: str, commit: str) -> Path:
    """The cached tarball of `commit`, downloaded on first use."""
    path = cache_dir() / f"{repo.replace('/', '_')}-{commit}.tar.gz"
    if path.is_file():
        return path
    url = f"https://codeload.github.com/{repo}/tar.gz/{commit}"
    print(f"download   : {url}")
    path.parent.mkdir(parents=True, exist_ok=True)
    part = path.with_suffix(".part")
    start = time.monotonic()
    request = urllib.request.Request(url, headers={"User-Agent": "dss-rs-corpus-fetch"})
    try:
        with urllib.request.urlopen(request, timeout=READ_TIMEOUT_S) as resp, open(part, "wb") as out:
            while chunk := resp.read(1 << 20):
                out.write(chunk)
                if time.monotonic() - start > DOWNLOAD_DEADLINE_S:
                    raise TimeoutError(f"not done after {DOWNLOAD_DEADLINE_S} s")
    except (OSError, TimeoutError) as e:
        part.unlink(missing_ok=True)
        fail(f"download of {url} failed: {e}\n  check the network, or put the tarball at {path}")
    os.replace(part, path)
    return path


def normalize(data: bytes) -> bytes:
    """LF line ends for a text file, the bytes as they are for any other."""
    if b"\0" in data or LONE_CR.search(data):
        return data
    return data.replace(b"\r\n", b"\n")


def unpack(path: Path) -> dict[str, bytes]:
    """Every file of the tarball, by its path below the top directory."""
    files: dict[str, bytes] = {}
    try:
        with tarfile.open(path, "r:gz") as tar:
            for member in tar:
                if member.isdir():
                    continue
                parts = PurePosixPath(member.name).parts[1:]
                if not member.isfile() or not parts or ".." in parts:
                    fail(f"{path}: refusing tar member {member.name!r}")
                data = tar.extractfile(member)
                if data is None:
                    fail(f"{path}: unreadable tar member {member.name!r}")
                files["/".join(parts)] = normalize(data.read())
    except (OSError, tarfile.TarError, EOFError) as e:
        fail(f"{path} is not a readable tarball ({e}): delete it and run again")
    if not files:
        fail(f"{path} holds no file: delete it and run again")
    return files


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_sums() -> dict[str, str]:
    """The listed digest of every file, by its path below the tree."""
    sums: dict[str, str] = {}
    try:
        lines = SUMS.read_text(encoding="utf-8").splitlines()
    except OSError as e:
        fail(f"cannot read {SUMS}: {e}")
    for n, line in enumerate(lines, 1):
        sha, sep, name = line.partition("  ")
        if not sep or len(sha) != 64 or not name.startswith(SUMS_PREFIX):
            fail(f"{SUMS}:{n}: not a `<sha256>  {SUMS_PREFIX}<path>` line")
        sums[name.removeprefix(SUMS_PREFIX)] = sha
    return sums


def write_sums(files: dict[str, bytes]) -> None:
    text = "".join(f"{digest(files[p])}  {SUMS_PREFIX}{p}\n" for p in sorted(files))
    SUMS.write_text(text, encoding="utf-8", newline="\n")


def check(files: dict[str, bytes], sums: dict[str, str]) -> None:
    missing = sorted(set(sums) - set(files))
    extra = sorted(set(files) - set(sums))
    changed = sorted(p for p in set(files) & set(sums) if digest(files[p]) != sums[p])
    if missing or extra or changed:
        shown = [f"  listed, not in the tarball: {p}" for p in missing[:5]]
        shown += [f"  in the tarball, not listed: {p}" for p in extra[:5]]
        shown += [f"  digest differs: {p}" for p in changed[:5]]
        fail(
            f"the tarball does not match {SUMS} ({len(missing)} missing, {len(extra)} extra, "
            f"{len(changed)} changed); after a pin change run with --rehash\n" + "\n".join(shown)
        )


def write_tree(files: dict[str, bytes], commit: str) -> tuple[list[str], list[str]]:
    """Make the tree hold exactly `files` and the marker: `(written, removed)`."""
    written: list[str] = []
    removed: list[str] = []
    errors: list[str] = []
    for rel, data in files.items():
        out = DST / rel
        try:
            if out.is_file() and out.read_bytes() == data:
                continue
            out.parent.mkdir(parents=True, exist_ok=True)
            out.write_bytes(data)
            written.append(rel)
        except OSError as e:
            errors.append(f"  write {out}: {e}")
    keep = set(files) | {MARKER}
    for p in sorted(DST.rglob("*"), reverse=True):
        rel = p.relative_to(DST).as_posix()
        try:
            if p.is_dir():
                if not any(p.iterdir()):
                    p.rmdir()
            elif rel not in keep:
                p.unlink()
                removed.append(rel)
        except OSError as e:
            errors.append(f"  remove {p}: {e}")
    if errors:
        fail("the tree could not be brought to the pin (a locked file?):\n" + "\n".join(errors))
    (DST / MARKER).write_text(commit + "\n", encoding="utf-8", newline="\n")
    return written, removed


def main() -> None:
    args = sys.argv[1:]
    if args not in ([], ["--rehash"]):
        fail("usage: vendor.py [--rehash]")
    repo, commit = load_pin()
    files = unpack(tarball(repo, commit))
    if args == ["--rehash"]:
        write_sums(files)
        print(f"wrote      : {SUMS} ({len(files)} files)")
    else:
        check(files, read_sums())
    DST.mkdir(parents=True, exist_ok=True)
    written, removed = write_tree(files, commit)
    print(f"tree       : {DST} at {repo}@{commit[:12]}")
    print(f"files      : {len(files)}, written {len(written)}, removed {len(removed)}")
    for verb, names in (("written", written), ("removed", removed)):
        for rel in names[:20]:
            print(f"  {verb} {rel}")
        if len(names) > 20:
            print(f"  ... {len(names) - 20} more {verb}")


if __name__ == "__main__":
    main()

"""DSS command/option coverage over the vendored corpus (PORTING_PLAN Phase 8;
PHASE8_PLAN WP8.6 step 1).

Enumerates every executive command (and every `Set`/`Solve` option) used across
the vendored corpus `tests/corpus/electricdss-tst`, cross-references the set the
Rust executive actually dispatches (`crates/dss-core/src/exec/tables.rs` +
`command.rs` for commands, `opt::` references across `exec/` for options), and
prints the exact unported tail. Read-only, stdlib-only, deterministic (sorted
output; counts are file-order independent).

Command-name matching replicates `TCommandList` (Shared/Command.pas — the port
in `support/command_list/mod.rs`): case-insensitive full names first, then every
proper prefix in registration order, first owner wins. A first token containing
`=` before any whitespace is a property assignment (`Line.L1.Length=2`), not a
command, exactly as `ProcessCommand` sees it (ParamName non-empty => pointer 0).

Usage:
    python tools/cmd_coverage.py
"""

from __future__ import annotations

import re
import sys
from collections import Counter
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
CORPUS = REPO_ROOT / "tests" / "corpus" / "electricdss-tst"
EXEC_DIR = REPO_ROOT / "crates" / "dss-core" / "src" / "exec"


# --------------------------------------------------------------------------
# Rust-side extraction
# --------------------------------------------------------------------------

def parse_string_table(src: str, name: str) -> list[str]:
    """Extract a `pub(crate) const NAME: &[&str] = &[ "…", … ];` table."""
    m = re.search(rf"{name}: &\[&str\] = &\[(.*?)\];", src, re.S)
    if not m:
        sys.exit(f"table {name} not found in tables.rs")
    return re.findall(r'"([^"]+)"', m.group(1))


def parse_ordinals(src: str, module: str) -> dict[str, int]:
    """Extract `pub const IDENT: usize = N;` from `pub(crate) mod cmd/opt`."""
    m = re.search(rf"mod {module} \{{(.*?)\n\}}", src, re.S)
    if not m:
        sys.exit(f"mod {module} not found in tables.rs")
    return {
        ident: int(num)
        for ident, num in re.findall(
            r"pub const (\w+): usize = (\d+);", m.group(1)
        )
    }


def dispatched_commands() -> tuple[list[str], dict[int, str]]:
    """Return (EXEC_COMMANDS, {ordinal: status}) where status is `ported` or
    `recognized-not-ported` (an explicit `not_ported_command` arm). Ordinals
    absent from the map fall through to the catch-all `not_ported_command`."""
    tables = (EXEC_DIR / "tables.rs").read_text(encoding="utf-8")
    command_rs = (EXEC_DIR / "command.rs").read_text(encoding="utf-8")
    exec_commands = parse_string_table(tables, "EXEC_COMMANDS")
    cmd_ordinals = parse_ordinals(tables, "cmd")

    status: dict[int, str] = {}
    # Match arms: one or more `cmd::IDENT` alternatives before `=>`; the body
    # runs to the next arm (or a brace close). An arm whose body calls
    # `not_ported_command` is recognized-but-unported.
    arm_re = re.compile(r"((?:cmd::\w+\s*\|\s*)*cmd::\w+)(?:\s*if[^=]*)?\s*=>")
    arms = list(arm_re.finditer(command_rs))
    for i, m in enumerate(arms):
        body_end = arms[i + 1].start() if i + 1 < len(arms) else len(command_rs)
        body = command_rs[m.end():body_end]
        arm_status = (
            "recognized-not-ported" if "not_ported_command" in body else "ported"
        )
        for ident in re.findall(r"cmd::(\w+)", m.group(1)):
            ordinal = cmd_ordinals.get(ident)
            if ordinal is None:
                sys.exit(f"cmd::{ident} used in command.rs but not in tables.rs")
            # An ordinal may appear in several arms (e.g. `SET if no circuit`
            # and the post-circuit `SET`); ported anywhere wins.
            if status.get(ordinal) != "ported":
                status[ordinal] = arm_status
    # The early non-match dispatch (`if pointer == cmd::COMPILE || pointer ==
    # cmd::REDIRECT { self.do_redirect(…) }`) is ported too.
    for ident in re.findall(r"pointer == cmd::(\w+)", command_rs):
        ordinal = cmd_ordinals.get(ident)
        if ordinal is not None:
            status[ordinal] = "ported"
    return exec_commands, status


def dispatched_options() -> tuple[list[str], set[int]]:
    """Return (EXEC_OPTIONS, set of implemented option ordinals): every
    `opt::IDENT` referenced anywhere under `exec/` counts as implemented."""
    tables = (EXEC_DIR / "tables.rs").read_text(encoding="utf-8")
    exec_options = parse_string_table(tables, "EXEC_OPTIONS")
    opt_ordinals = parse_ordinals(tables, "opt")
    used: set[int] = set()
    for p in sorted(EXEC_DIR.rglob("*.rs")):
        if p.name == "tables.rs":
            continue
        for ident in re.findall(r"opt::(\w+)", p.read_text(encoding="utf-8")):
            if ident in opt_ordinals:
                used.add(opt_ordinals[ident])
    return exec_options, used


# --------------------------------------------------------------------------
# TCommandList lookup (full names + first-free proper prefixes)
# --------------------------------------------------------------------------

def build_lookup(names: list[str]) -> dict[str, int]:
    lookup: dict[str, int] = {}
    for i, name in enumerate(names):
        lookup.setdefault(name.lower(), i)
    for i, name in enumerate(names):
        key = name.lower()
        for j in range(1, len(key)):
            lookup.setdefault(key[:j], i)
    return lookup


# --------------------------------------------------------------------------
# Corpus scan
# --------------------------------------------------------------------------

FIRST_TOKEN = re.compile(r'[^\s=,]+')


def scan_corpus(
    cmd_lookup: dict[str, int], opt_lookup: dict[str, int], set_solve: set[int]
) -> tuple[Counter, Counter, Counter]:
    """Walk every corpus .dss file; return (command counts by ordinal-index,
    option counts by ordinal-index for Set/Solve lines, unknown first tokens)."""
    cmd_counts: Counter = Counter()
    opt_counts: Counter = Counter()
    unknown: Counter = Counter()
    files = sorted(CORPUS.rglob("*.dss")) + sorted(
        p for p in CORPUS.rglob("*.DSS") if p.suffix == ".DSS"
    )
    if not files:
        sys.exit(f"no .dss files under {CORPUS}")
    for f in files:
        in_block_comment = False
        for raw in f.read_text(encoding="latin-1").splitlines():
            line = raw.lstrip("﻿").strip()
            if in_block_comment:
                if "*/" in line:
                    in_block_comment = False
                continue
            if line.startswith("/*"):
                # DoRedirect: `/*` only recognized at line start.
                if "*/" not in line:
                    in_block_comment = True
                continue
            if not line or line.startswith(("!", "//")):
                continue
            m = FIRST_TOKEN.match(line)
            if not m:
                continue
            token = m.group(0)
            rest = line[m.end():]
            if rest.startswith("="):
                # `name=value` first param → ProcessCommand pointer 0 →
                # a property assignment, not a command.
                continue
            idx = cmd_lookup.get(token.lower())
            if idx is None:
                unknown[token.lower()] += 1
                continue
            cmd_counts[idx] += 1
            if idx in set_solve:
                # Collect `name=` option tokens on Set/Solve lines.
                for name in re.findall(r'([\w%]+)\s*=', rest):
                    oidx = opt_lookup.get(name.lower())
                    if oidx is not None:
                        opt_counts[oidx] += 1
    return cmd_counts, opt_counts, unknown


# --------------------------------------------------------------------------
# Report
# --------------------------------------------------------------------------

def main() -> None:
    exec_commands, cmd_status = dispatched_commands()
    exec_options, opt_impl = dispatched_options()
    cmd_lookup = build_lookup(exec_commands)
    opt_lookup = build_lookup(exec_options)
    set_solve = {
        exec_commands.index("Set"),
        exec_commands.index("Solve"),
    }
    cmd_counts, opt_counts, unknown = scan_corpus(cmd_lookup, opt_lookup, set_solve)

    def cmd_state(i: int) -> str:
        return cmd_status.get(i + 1, "unported")  # ordinal = index + 1

    print(f"corpus: {CORPUS.relative_to(REPO_ROOT)}")
    print()
    print("== commands used in the corpus ==")
    print(f"{'command':<20} {'uses':>7}  status")
    for i, n in sorted(cmd_counts.items(), key=lambda kv: (-kv[1], exec_commands[kv[0]].lower())):
        print(f"{exec_commands[i]:<20} {n:>7}  {cmd_state(i)}")

    tail = sorted(
        (exec_commands[i], n)
        for i, n in cmd_counts.items()
        if cmd_state(i) != "ported"
    )
    print()
    print(f"== UNPORTED TAIL: corpus-used commands not dispatched ({len(tail)}) ==")
    for name, n in tail:
        print(f"{name:<20} {n:>7}")

    unused_unported = sorted(
        exec_commands[i]
        for i in range(len(exec_commands))
        if i not in cmd_counts and cmd_state(i) != "ported"
    )
    print()
    print(f"== unported commands with NO corpus use ({len(unused_unported)}) ==")
    print(", ".join(unused_unported))

    opt_tail = sorted(
        (exec_options[i], n)
        for i, n in opt_counts.items()
        if (i + 1) not in opt_impl
    )
    print()
    print("== Set/Solve options used in the corpus ==")
    print(f"{'option':<24} {'uses':>7}  status")
    for i, n in sorted(opt_counts.items(), key=lambda kv: (-kv[1], exec_options[kv[0]].lower())):
        state = "ported" if (i + 1) in opt_impl else "unported"
        print(f"{exec_options[i]:<24} {n:>7}  {state}")
    print()
    print(f"== UNPORTED option tail ({len(opt_tail)}) ==")
    for name, n in opt_tail:
        print(f"{name:<24} {n:>7}")

    print()
    # Mostly bus names from coordinate DATA files that carry a `.dss`
    # extension (e.g. Buscoords payloads), not real commands — summarize.
    print(
        f"== unknown first tokens (not a command; {len(unknown)} distinct, "
        f"{sum(unknown.values())} total; top 25) =="
    )
    top = sorted(unknown.items(), key=lambda kv: (-kv[1], kv[0]))[:25]
    for tok, n in top:
        print(f"{tok:<32} {n:>7}")


if __name__ == "__main__":
    main()

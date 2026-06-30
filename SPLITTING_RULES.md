# SPLITTING_RULES — module-splitting rules & verification protocol

Rules for splitting a large or multi-concern module into a directory module, or
extracting its tests, **without changing behavior**. Referenced from `CLAUDE.md`
(*Module layout*). This is a pure code-organization refactor under **PORTING_PLAN
§1.4** (*"later phases may freely refactor earlier code; the regression test suite
is the only contract"*): no numeric behavior change, no goldens regenerated,
`TODO(compat)`/`NOT_PORTED` markers preserved **verbatim**.

> Executed splits are recorded in git history, not here
> (`git log --grep '^refactor: split'` / `'^refactor: extract'`).

---

## 1. Rules of engagement

1. **One split at a time.** Finish a split → verify byte-faithfulness (§3) → run
   the full gate (§4) → **stop**. Do not start the next until the current one is
   green and verified.
2. **Never delete the original until verified.** Snapshot the pre-split content
   from git (it is the committed contract); compare new-vs-old with the normalized
   diff in §3. Only after **zero logic lines removed** is confirmed do you remove
   the original.
3. **The gate must be green before any commit** (§4).
4. **Commit only on explicit user request.** Each split is one self-contained
   commit (`refactor: split <file> into submodules`), gate-green, no goldens
   touched.
5. **Do not touch a file an active WP is mid-editing.** Splitting a file under
   live change invites merge friction — defer it to a phase/WP boundary.
6. **No `TODO(compat)`/`NOT_PORTED` "fixes."** They stay greppable and
   byte-identical across the move.

---

## 2. When to split, and what to leave alone

**Split when** a module grows large (≳350–450 lines) or mixes concerns: convert
`foo.rs` → a directory module `foo/mod.rs` + concern submodules (`accessors`,
`edit`, `solve`, `compute`, …). `mod.rs` keeps the struct, the `define_properties!`
invocation, the construction/lifecycle, and the orchestration; each submodule owns
exactly one concern.

**Tests:** inline `#[cfg(test)] mod tests { … }` is the default (CLAUDE.md). Extract
to a sibling `tests.rs` (`#[cfg(test)] mod tests;`) **only when the file is large**;
the declaration goes **right after the module doc**, before the other `mod`/`use`
lines. A same-depth extraction (`foo.rs`'s inline tests → `foo/tests.rs`) keeps the
test block's `use super::*;` unchanged.

**Leave alone:** small single-file modules (inline tests there are pure overhead),
and Tier-2 cohesive single-responsibility files that read fine whole — splitting
buys little. Separate `tests.rs` files are a size-driven accommodation, **not** a
target everything must converge to.

---

## 3. Verification — run AFTER every split, BEFORE deleting the original

The invariant: **the set of executable lines is identical before and after** — the
split only *relocates* code, never edits it.

### 3a. Normalized line-set diff (every split)

Capture the original from git, normalize both sides (strip indentation + visibility
qualifiers; drop `use`/`mod`/comment/attribute/blank lines), sort, and diff:

```bash
norm() {
  sed -E 's/^[[:space:]]+//; s/^pub\(crate\) //; s/^pub\(super\) //; s/^pub\(in [^)]*\) //; s/^pub //' \
  | grep -vE '^(use |mod |pub use |pub mod |#\[|#!\[|//|/\*|\*|$)' \
  | sort
}
OLD=path/to/file.rs                 # the file being split (path at HEAD)
NEWGLOB='path/to/file/*.rs'         # the new submodules

git show "HEAD:$OLD" | norm > /tmp/split_old.txt
cat $NEWGLOB         | norm > /tmp/split_new.txt
echo "### REMOVED (MUST be empty):"; comm -23 /tmp/split_old.txt /tmp/split_new.txt
echo "### ADDED (only benign scaffolding):"; comm -13 /tmp/split_old.txt /tmp/split_new.txt
```

- **REMOVED must be empty.** A single removed logic line = something was lost → stop
  and fix before deleting the original.
- **ADDED is allowed only** for benign scaffolding the filter doesn't strip: extra
  `impl X {` / closing `}` from splitting one `impl`, a multi-line `use {…}`/signature
  reformatted to one line, the relocated struct/field/`fn` wiring of an extraction.
  Anything semantic in ADDED = a real change → stop.

When a reshape **rewrites** the struct-assembly (e.g. extracting one constructor into
by-domain helpers that return id structs), §3a's REMOVED cannot stay empty. Then
supplement it with a fidelity check on the substantive lines — e.g. every
constructor call, mutation, and string literal byte-identical old↔new (sorted diff)
plus a count check — and lean on the behavioral gate (§4).

### 3b. Test-identity check (whenever tests move)

```bash
git show "HEAD:<old>" | grep -oE '^\s*fn [a-z_0-9]+' | sed 's/^[[:space:]]*//' | sort > /tmp/t_old.txt
cat <new-glob>        | grep -oE '^\s*fn [a-z_0-9]+' | sed 's/^[[:space:]]*//' | sort > /tmp/t_new.txt
diff /tmp/t_old.txt /tmp/t_new.txt && echo "OK: identical fn set"
```

The `#[test]` set and the `test result: ok. N passed` count must equal the
pre-split values. Normalize visibility first — a helper promoted to `pub(super) fn`
won't match a bare `^fn` grep.

---

## 4. The gate (after each split)

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

All three green, including the live-oracle `corpus_live` gate — a pure file move
cannot affect it, which is the point: if it moves, something leaked.

---

## 5. Known gotchas

- **super-remap (file→dir).** `foo.rs`'s `use super::*` (super = parent) must become
  `use crate::…::*;` in submodules whose super is now `foo`. A *same-depth* test
  extraction (`foo.rs` inline tests → `foo/tests.rs`) does **not** need this — `super`
  is unchanged.
- **Helper visibility.** Cross-submodule helpers move to a shared file as
  `pub(super)`; group-local helpers stay private where they're used.
- **cfg gating.** The single `#[cfg(test)] mod tests;` in the parent gates the whole
  subtree; submodules need no `#[cfg(test)]` of their own.
- **Trait-in-scope break.** Moving `impl Trait for X` **and its `use …Trait`** out of
  `mod.rs` removes the trait from `mod.rs`'s scope, so a sibling `tests.rs` using
  `use super::*` and calling trait methods fails E0599 — add `use crate::…::Trait;`
  to the test file.
- **Line endings.** The repo has mixed CRLF/LF files (git autocrlf). A `\n`-only
  regex (perl/sed) silently no-ops on CRLF files — match `\r?\n` and detect each
  file's ending, or operate line-based. Git normalizes to LF on commit, so verify
  real size with `git diff --stat`, not a raw `diff` (which counts line-ending
  noise).
- **Byte-faithful extraction.** Snapshot the original (`git show HEAD:…`), extract
  exact line ranges with `sed -n 'A,Bp'` (no manual transcription), prepend the
  per-file `use` header, then `cargo fmt` to normalize indentation/ordering.

# The build/test queue (`gatelock.py`)

`gatelock.py` is a first-come-first-served queue for the cargo commands of the gate on a
shared Windows machine, so parallel lanes and the coordinator never compile or run tests
past what the cores can carry. There is one queue per main checkout and its worktrees:
every copy of the script, in the main checkout or in a worktree of it, queues on the same
directory. Another clone joins that queue only through `GATELOCK_ROOT`.

## Usage

```
python tools/gate/gatelock.py --label "<who>" -- cargo <args>
```

Run from the root of the tree being built, with the cargo arguments exactly as they would
be typed. The wrapper exits with the command's exit code.

## Slots

| slot | capacity | commands |
|---|---|---|
| `build` | 2 | `cargo clippy`, `build`, `check`, `doc`, `run`, `rustc`, `rustdoc`, `nextest list`/`archive`, and the compile phase of a test run |
| `test` | 1 | `cargo nextest run`, `cargo test`, `cargo bench` |

- A `cargo nextest run`, `cargo test` or `cargo bench` is split: the same command with
  `--no-run` first, under the build slot, then the run itself under the test slot, so the
  test slot is never held while compiling.
- `cargo test --doc` cannot take `--no-run` and runs whole under the test slot.
- A build-slot command gets `CARGO_BUILD_JOBS=8` unless the caller set it.
- Every other command (`cargo fmt`, `cargo metadata`, anything that is not cargo) runs
  unqueued.

## Files

The queue root is `<main checkout>/tmp/gatelock/` (`tmp/` is gitignored), the main
checkout being the parent of git's common directory for the script's own directory:

- `board.txt` - who holds a slot, who waits and since when, rewritten on every change;
- `board.log` - one line per finished command with its slot, ticket number, label, wait
  and hold seconds, exit code and the command (cut at 200 characters), and one line per
  purged dead ticket and per failed job assignment;
- `tickets/` - the queue itself. Each waiter keeps an OS byte lock on its own ticket for its
  whole life, and the next wrapper that reads the queue purges a ticket whose lock it can
  take (a dead process). Tickets are never deleted by hand.

The command runs inside a Windows job object, so killing the wrapper also kills its cargo,
rustc, test binaries and `epri-worker`s.

## Environment

- `GATELOCK_ROOT` - the queue directory itself; overrides the git lookup. A queued command
  looks the root up before it takes a ticket: when `GATELOCK_ROOT` is unset and the lookup
  fails, the wrapper runs nothing and exits non-zero. `GATELOCK_OFF=1` and the unqueued
  commands never need the root.
- `GATELOCK_OFF=1` - run the command unqueued.

Windows only: on another platform the wrapper says so on stderr and runs the command
unqueued, without a job object.

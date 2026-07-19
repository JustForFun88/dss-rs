"""Line-JSON client for the `epri-worker` Rust bridge (`crates/dss-epri`) —
the functional-parity replacement for the retired AltDSS Oddie bridge.

The worker process loads the vendored official EPRI r4133
`tools/opendss/bin/r4133/OpenDSSDirect.dll`, asserts the version pin
(`revisions.json`), and serves `exec` (one executive command, error-checked,
actor-synchronized) / `read` (one API property read, error-checked) / `chdir`
requests over stdin/stdout. See `crates/dss-epri/src/script.rs` for the
dispatch table and `crates/dss-epri/tests/protocol.rs` for the smoke that pins
the read shapes and the event-log format.

Two surfaces:

- :class:`EpriWorker` — the raw protocol client (spawn, request, exec, read).
- :class:`EpriEngine` — a thin `IOddieDSS`-shaped shim (``d.Text.Command``,
  ``d.ActiveCircuit.Solution`` …) so drivers written against the old Oddie API
  (`tools/golden/gen_protection.py::build`, `gen_checkpoints.capture_element`)
  run unchanged over the Rust bridge, issuing the **same per-property DLL call
  sequence** the Oddie bridge did (one read per protocol request).

Binary resolution mirrors the gate (`corpus_gate/engines.rs::epri_worker_bin`):
`DSS_EPRI_WORKER` env override -> `target/release/epri-worker(.exe)` ->
`target/debug/...` -> one `cargo build -p dss-epri --bin epri-worker`.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]


def find_worker() -> Path:
    """Resolve (building if needed) the `epri-worker` binary."""
    env = os.environ.get("DSS_EPRI_WORKER")
    if env:
        p = Path(env)
        if not p.is_file():
            sys.exit(f"DSS_EPRI_WORKER points at a missing file: {p}")
        return p
    exe = "epri-worker.exe" if os.name == "nt" else "epri-worker"
    for profile in ("release", "debug"):
        p = REPO_ROOT / "target" / profile / exe
        if p.is_file():
            return p
    print("epri-worker not built — running `cargo build -p dss-epri --bin epri-worker`…")
    r = subprocess.run(
        ["cargo", "build", "-p", "dss-epri", "--bin", "epri-worker"],
        cwd=REPO_ROOT,
    )
    if r.returncode != 0:
        sys.exit("`cargo build -p dss-epri --bin epri-worker` failed")
    p = REPO_ROOT / "target" / "debug" / exe
    if not p.is_file():
        sys.exit(f"epri-worker still missing after build: {p}")
    return p


class EpriWorker:
    """One persistent `epri-worker` process; every method raises on `ok:false`
    (mirroring dss-python's raise-on-error after each command/read)."""

    def __init__(self, binary: Path | None = None):
        self._proc = subprocess.Popen(
            [str(binary or find_worker())],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=None,  # inherit: the `epri-worker ready: …` banner stays visible
            text=True,
            encoding="utf-8",
        )
        pong = self.request({"cmd": "ping"})
        oracle = pong.get("oracle", {})
        if not oracle.get("epri") or oracle.get("rev") != "r4133":
            sys.exit(f"epri-worker did not answer as the r4133 EPRI bridge: {pong}")
        self.oracle = oracle
        self.version: str = str(oracle.get("engine", ""))

    def request(self, msg: dict) -> dict:
        """Send one request, return the reply's `result`; raise on `ok:false`."""
        assert self._proc.stdin and self._proc.stdout
        self._proc.stdin.write(json.dumps(msg) + "\n")
        self._proc.stdin.flush()
        line = self._proc.stdout.readline()
        if not line.strip():
            raise RuntimeError(f"epri-worker died on {msg!r}")
        reply = json.loads(line)
        if not reply.get("ok"):
            raise RuntimeError(f"epri-worker error on {msg!r}: {reply.get('error')}")
        return reply.get("result")

    def exec(self, text: str) -> str:
        """One executive command (`Text.Command` parity); returns the reply."""
        return self.request({"cmd": "exec", "text": text})["reply"]

    def read(self, what: str, **kw):
        """One API property read (see `script.rs` for the `what` table)."""
        return self.request({"cmd": "read", "what": what, **kw})

    def chdir(self, path) -> None:
        """chdir the worker process (relative `File=` resolution parity with the
        old drivers' `os.chdir` before Oddie commands)."""
        self.request({"cmd": "chdir", "dir": str(path)})

    def close(self) -> None:
        try:
            assert self._proc.stdin
            self._proc.stdin.write(json.dumps({"cmd": "quit"}) + "\n")
            self._proc.stdin.flush()
        except Exception:
            pass
        self._proc.wait(timeout=30)


# ---------------------------------------------------------------------------
# IOddieDSS-shaped shim (just enough surface for the golden-regen drivers).
# ---------------------------------------------------------------------------


class _Text:
    def __init__(self, w: EpriWorker):
        self._w = w
        self.Result = ""

    @property
    def Command(self):  # pragma: no cover - write-only in practice
        return None

    @Command.setter
    def Command(self, cmd: str) -> None:
        self.Result = self._w.exec(cmd)


class _Solution:
    def __init__(self, w: EpriWorker):
        self._w = w

    @property
    def dblHour(self) -> float:
        return self._w.read("dbl_hour")

    @property
    def Iterations(self) -> int:
        return self._w.read("iterations")

    @property
    def Converged(self) -> bool:
        return self._w.read("converged")

    @property
    def EventLog(self) -> list[str]:
        return self._w.read("eventlog")


class _ActiveElement:
    """Powers is read before Currents by `capture_element` — each property get
    is one protocol request = one DLL read, same order as Oddie."""

    def __init__(self, w: EpriWorker):
        self._w = w

    @property
    def Powers(self) -> list[float]:
        return self._w.read("element_powers")

    @property
    def Currents(self) -> list[float]:
        return self._w.read("element_currents")


class _Circuit:
    def __init__(self, w: EpriWorker):
        self._w = w
        self.Solution = _Solution(w)
        self.ActiveCktElement = _ActiveElement(w)

    def SetActiveElement(self, name: str) -> None:
        self._w.read("set_active_element", name=name)

    @property
    def YNodeVarray(self) -> list[float]:
        return self._w.read("ynode_varray")

    @property
    def YNodeOrder(self) -> list[str]:
        return self._w.read("ynode_order")

    @property
    def AllElementNames(self) -> list[str]:
        return self._w.read("all_element_names")


class EpriEngine:
    """`IOddieDSS`-shaped facade over :class:`EpriWorker` (r4133 only)."""

    def __init__(self, binary: Path | None = None):
        self.worker = EpriWorker(binary)
        self.Text = _Text(self.worker)
        self.ActiveCircuit = _Circuit(self.worker)
        self.Version = self.worker.version
        self.AllowForms = False  # the worker already runs DSSI(8,0) NoForms

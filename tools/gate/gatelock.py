"""Machine-wide FIFO queue for cargo builds and test runs.

One queue per main checkout and its worktrees: every copy of this script in the main
checkout or in a worktree of it queues on the same directory. Another clone joins it only
through GATELOCK_ROOT.

Usage:  python tools/gate/gatelock.py --label "<who>" -- cargo <args>

Slots (capacity in SLOTS):
  build  - cargo clippy / build / check / doc / run / nextest list, and the compile phase
           of a test run (`--no-run`)
  test   - executing tests: cargo nextest run, cargo test, cargo bench (doctests included)
A `cargo nextest run ...` or `cargo test ...` is split automatically: first the same
command with `--no-run` under the build slot, then the real run under the test slot, so
the test slot is never held while compiling. `cargo test --doc` cannot be split (cargo
refuses `--no-run` with `--doc`) and runs whole under the test slot. Any other command
(fmt, metadata, ...) runs without a lock.

Liveness: each waiter/holder keeps an OS byte-lock on its own ticket file for its whole
life. A ticket whose lock can be taken belongs to a dead process and is purged, so a
killed agent never blocks the queue. The command runs inside a Windows job object that
dies with the wrapper, so killing the wrapper also kills its cargo, rustc, test binaries
and epri-workers instead of leaving them running outside the queue.
The queue root is <main checkout>/tmp/gatelock/, the main checkout being the parent of
the directory `git -C <this script's directory> rev-parse --path-format=absolute
--git-common-dir` prints (GIT_DIR, GIT_COMMON_DIR and GIT_WORK_TREE removed from git's
environment). GATELOCK_ROOT names the queue root itself and overrides the lookup. The
root is looked up only when a command is queued; when the lookup fails and GATELOCK_ROOT
is unset, nothing runs and the wrapper exits non-zero. The tickets sit under
<root>/tickets/ and are never deleted by hand.
The board (who holds, who waits, since when) is rewritten on every change:
<root>/board.txt; events are appended to <root>/board.log (wait and hold seconds per run).
Board and log writes are best-effort and never fail a run.
Exit code = the command's exit code. GATELOCK_OFF=1 runs the command unqueued.
Windows only: on another platform the wrapper says so on stderr and runs the command
unqueued, without a job object.
"""
import json
import os
import re
import subprocess
import sys
import time

WINDOWS = sys.platform == "win32"
if WINDOWS:
    import ctypes
    import msvcrt
    from ctypes import wintypes

ROOT = QDIR = BOARD = LOG = None  # set by use_root()
SLOTS = {"build": 2, "test": 1}
BUILD_JOBS = "8"  # two concurrent builds share the 16 cores
POLL = 3.0
TICKET_RE = re.compile(r"^(\d{9})\.tk$")

BUILD_SUBS = {"clippy", "build", "b", "check", "c", "doc", "d", "run", "r", "rustc", "rustdoc"}
# cargo global options that take a value (the value is skipped when locating the subcommand)
VALUE_OPTS = {"--config", "-Z", "-C", "--color", "--manifest-path"}


def now():
    return time.strftime("%Y-%m-%d %H:%M:%S")


def best_effort(fn, tries=20):
    """Board/log writes: a reader holding the file open must never fail a run."""
    for _ in range(tries):
        try:
            fn()
            return
        except OSError:
            time.sleep(0.05)


def find_root():
    """(root, None), or (None, why) when GATELOCK_ROOT is unset and git gives no main checkout."""
    root = os.environ.get("GATELOCK_ROOT")
    if root:
        return root, None
    here = os.path.dirname(os.path.abspath(__file__))
    env = {k: v for k, v in os.environ.items()
           if k.upper() not in ("GIT_DIR", "GIT_COMMON_DIR", "GIT_WORK_TREE")}
    try:
        r = subprocess.run(["git", "-C", here, "rev-parse", "--path-format=absolute", "--git-common-dir"],
                           env=env, capture_output=True, text=True)
    except OSError as e:
        return None, f"git could not be run ({e})"
    if r.returncode != 0:
        return None, f"git rev-parse failed in {here} (exit {r.returncode}): {r.stderr.strip()}"
    lines = r.stdout.splitlines()
    if len(lines) != 1 or not os.path.isabs(lines[0]) or not os.path.isdir(lines[0]):
        return None, f"git rev-parse in {here} printed {r.stdout!r}, not one absolute directory"
    return os.path.join(os.path.dirname(os.path.normpath(lines[0])), "tmp", "gatelock"), None


def use_root(root):
    global ROOT, QDIR, BOARD, LOG
    ROOT = root
    QDIR = os.path.join(root, "tickets")
    BOARD = os.path.join(root, "board.txt")
    LOG = os.path.join(root, "board.log")


def require_root():
    """Set the queue root before a ticket is taken; run nothing without one."""
    if ROOT is not None:
        return
    root, why = find_root()
    if root is None:
        print(f"[gatelock] cannot locate the queue root: {why}\n"
              "[gatelock] set GATELOCK_ROOT to the queue directory, or GATELOCK_OFF=1 to run "
              "the command unqueued; nothing was run", file=sys.stderr, flush=True)
        finish(2)
    use_root(root)


class Meta:
    """Short critical section around the queue directory."""

    def __enter__(self):
        os.makedirs(QDIR, exist_ok=True)
        self.f = open(QDIR + "/meta.lock", "a+b")
        while True:
            try:
                self.f.seek(0)
                msvcrt.locking(self.f.fileno(), msvcrt.LK_NBLCK, 1)
                return self
            except OSError:
                time.sleep(0.05)

    def __exit__(self, *a):
        self.f.seek(0)
        msvcrt.locking(self.f.fileno(), msvcrt.LK_UNLCK, 1)
        self.f.close()


def alive_tickets(slot, mine=None):
    """Tickets of `slot` in FIFO order and whether a dead one was purged. Call under Meta."""
    d = f"{QDIR}/{slot}"
    os.makedirs(d, exist_ok=True)
    out, purged = [], False
    for name in sorted(n for n in os.listdir(d) if TICKET_RE.match(n)):
        p = f"{d}/{name}"
        if p == mine:
            out.append(p)
            continue
        try:
            f = open(p, "r+b")
        except OSError:
            continue
        try:
            msvcrt.locking(f.fileno(), msvcrt.LK_NBLCK, 1)
        except OSError:
            f.close()
            out.append(p)  # locked by a live process
            continue
        msvcrt.locking(f.fileno(), msvcrt.LK_UNLCK, 1)
        f.close()
        try:
            os.remove(p)
            purged = True
            log(f"purged dead ticket {slot}/{name}")
        except OSError:
            pass
    return out, purged


def next_number():
    """Ticket number: above both the counter and every ticket on disk. Call under Meta."""
    cnt = QDIR + "/counter"
    n = 0
    try:
        with open(cnt) as f:
            n = int(f.read().strip() or 0)
    except (OSError, ValueError):
        pass
    for slot in SLOTS:
        d = f"{QDIR}/{slot}"
        if os.path.isdir(d):
            for name in os.listdir(d):
                m = TICKET_RE.match(name)
                if m:
                    n = max(n, int(m.group(1)))
    n += 1
    with open(cnt, "w") as f:
        f.write(str(n))
    return n


def info(p):
    try:
        with open(p, "rb") as f:
            f.seek(1)
            return json.loads(f.read().decode("utf-8"))
    except Exception:
        return {}


def log(msg):
    if LOG is None:
        root, _ = find_root()
        if root is None:
            return
        use_root(root)

    def w():
        os.makedirs(os.path.dirname(LOG), exist_ok=True)
        with open(LOG, "a", encoding="utf-8") as f:
            f.write(f"{now()} {msg}\n")

    best_effort(w)


def board_text():
    """The board's text. Call under Meta, then publish() it after leaving Meta."""
    lines = [f"gate board, {now()}"]
    for slot, cap in SLOTS.items():
        t, _ = alive_tickets(slot)
        lines.append(f"[{slot}] capacity {cap}")
        for i, p in enumerate(t):
            j = info(p)
            state = "RUNNING" if i < cap else f"waiting #{i - cap + 1}"
            since = j.get("run_since") or j.get("queued")
            lines.append(f"  {state:11} {j.get('label', '?')}  since {since}  {j.get('cmd', '')[:140]}")
    return "\n".join(lines) + "\n"


def publish(text):
    """Write the board outside Meta, so a reader holding it open never stalls the queue."""

    def w():
        os.makedirs(os.path.dirname(BOARD), exist_ok=True)
        tmp = f"{BOARD}.{os.getpid()}.tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            f.write(text)
        try:
            os.replace(tmp, BOARD)
        except OSError:
            os.remove(tmp)
            raise

    best_effort(w, tries=5)


class Ticket:
    def __init__(self, slot, label, cmd):
        self.slot, self.label, self.cmd = slot, label, cmd
        self.rc = None
        self.n = None
        self.waited = 0.0
        self.t_run = None

    def _write(self, **extra):
        self.meta.update(extra)
        self.f.seek(1)
        self.f.truncate()
        self.f.write(json.dumps(self.meta).encode("utf-8"))
        self.f.flush()

    def __enter__(self):
        with Meta():
            self.n = next_number()
            self.path = f"{QDIR}/{self.slot}/{self.n:09d}.tk"
            os.makedirs(os.path.dirname(self.path), exist_ok=True)
            self.f = open(self.path, "w+b")
            self.f.write(b" ")
            self.f.flush()
            self.f.seek(0)
            msvcrt.locking(self.f.fileno(), msvcrt.LK_NBLCK, 1)
            self.meta = {"label": self.label, "pid": os.getpid(), "cmd": self.cmd, "queued": now()}
            self._write()
            board = board_text()
        publish(board)
        t0 = time.time()
        announced = False
        while True:
            board = None
            with Meta():
                tickets, purged = alive_tickets(self.slot, self.path)
                go = tickets.index(self.path) < SLOTS[self.slot]
                if go:
                    self._write(run_since=now())
                if go or purged:
                    board = board_text()
            if board:
                publish(board)
            if go:
                break
            if not announced:
                print(f"[gatelock] {self.label}: queued for slot '{self.slot}' "
                      f"(board: {BOARD})", file=sys.stderr, flush=True)
                announced = True
            time.sleep(POLL)
        self.waited = time.time() - t0
        self.t_run = time.time()
        return self

    def __exit__(self, *a):
        held = time.time() - self.t_run if self.t_run else 0.0
        with Meta():
            self.f.seek(0)
            try:
                msvcrt.locking(self.f.fileno(), msvcrt.LK_UNLCK, 1)
            except OSError:
                pass
            self.f.close()
            try:
                os.remove(self.path)
            except OSError:
                pass
            board = board_text()
        publish(board)
        log(f"{self.slot} #{self.n} {self.label} waited={self.waited:.0f}s held={held:.0f}s rc={self.rc} :: {self.cmd[:200]}")


# --- Windows job object: the command's process tree dies with this wrapper ---------------
if WINDOWS:
    class _IoCounters(ctypes.Structure):
        _fields_ = [(n, ctypes.c_ulonglong) for n in (
            "ReadOperationCount", "WriteOperationCount", "OtherOperationCount",
            "ReadTransferCount", "WriteTransferCount", "OtherTransferCount")]


    class _BasicLimits(ctypes.Structure):
        _fields_ = [("PerProcessUserTimeLimit", ctypes.c_longlong),
                    ("PerJobUserTimeLimit", ctypes.c_longlong),
                    ("LimitFlags", wintypes.DWORD),
                    ("MinimumWorkingSetSize", ctypes.c_size_t),
                    ("MaximumWorkingSetSize", ctypes.c_size_t),
                    ("ActiveProcessLimit", wintypes.DWORD),
                    ("Affinity", ctypes.c_size_t),
                    ("PriorityClass", wintypes.DWORD),
                    ("SchedulingClass", wintypes.DWORD)]


    class _ExtendedLimits(ctypes.Structure):
        _fields_ = [("BasicLimitInformation", _BasicLimits),
                    ("IoInfo", _IoCounters),
                    ("ProcessMemoryLimit", ctypes.c_size_t),
                    ("JobMemoryLimit", ctypes.c_size_t),
                    ("PeakProcessMemoryUsed", ctypes.c_size_t),
                    ("PeakJobMemoryUsed", ctypes.c_size_t)]


_JOB = None


def kill_on_close_job():
    """A job object with KILL_ON_JOB_CLOSE, held open for this process's life (or None)."""
    global _JOB
    if _JOB is not None:
        return _JOB
    try:
        k32 = ctypes.WinDLL("kernel32", use_last_error=True)
        k32.CreateJobObjectW.restype = wintypes.HANDLE
        k32.CreateJobObjectW.argtypes = [ctypes.c_void_p, wintypes.LPCWSTR]
        k32.SetInformationJobObject.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD]
        job = k32.CreateJobObjectW(None, None)
        if not job:
            return None
        lim = _ExtendedLimits()
        lim.BasicLimitInformation.LimitFlags = 0x2000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        if not k32.SetInformationJobObject(job, 9, ctypes.byref(lim), ctypes.sizeof(lim)):
            return None
        _JOB = (k32, job)
        return _JOB
    except Exception:
        return None


def call(argv, env=None):
    """subprocess.call, with the child placed in the kill-on-close job."""
    p = subprocess.Popen(argv, env=env)
    j = kill_on_close_job() if WINDOWS else None
    if j:
        k32, job = j
        k32.AssignProcessToJobObject.argtypes = [wintypes.HANDLE, wintypes.HANDLE]
        if not k32.AssignProcessToJobObject(job, int(p._handle)):
            log(f"warning: AssignProcessToJobObject failed ({ctypes.get_last_error()}) :: {argv[:3]}")
    try:
        return p.wait()
    except BaseException:
        p.kill()
        raise


def run(slot, label, argv, env=None):
    require_root()
    cmd = subprocess.list2cmdline(argv)
    with Ticket(slot, label, cmd) as t:
        t.rc = call(argv, env)
    return t.rc


def with_no_run(argv, i_sub):
    """argv with --no-run inserted before a `--` separator (test-binary args)."""
    if "--" in argv[i_sub:]:
        i = argv.index("--", i_sub)
        return argv[:i] + ["--no-run"] + argv[i:]
    return argv + ["--no-run"]


def subcommand(a):
    """Index of cargo's subcommand in argv `a` (a[0] = cargo), skipping +toolchain and globals."""
    i = 1
    while i < len(a):
        x = a[i]
        if x.startswith("+") or (x.startswith("-") and "=" in x):
            i += 1
        elif x in VALUE_OPTS:
            i += 2
        elif x.startswith("-"):
            i += 1
        else:
            return i
    return None


def finish(rc):
    """Exit with the command's code; a Windows NTSTATUS (e.g. 0xC0000005) keeps its bits."""
    rc = int(rc or 0)
    if rc >= 2 ** 31:
        rc -= 2 ** 32
    sys.stdout.flush()
    sys.stderr.flush()
    os._exit(rc)


def main():
    a = sys.argv[1:]
    label = "?"
    if a[:1] == ["--label"]:
        label, a = a[1], a[2:]
    if a[:1] == ["--"]:
        a = a[1:]
    if not a:
        sys.exit("usage: gatelock.py --label <who> -- cargo <args...>")
    if not WINDOWS:
        print(f"[gatelock] sys.platform is {sys.platform!r}, not 'win32': the queue needs Windows "
              "byte locks and a job object, so the command runs unqueued", file=sys.stderr, flush=True)
        finish(call(a))
    if os.environ.get("GATELOCK_OFF") == "1" or os.path.basename(a[0]).lower() not in ("cargo", "cargo.exe"):
        finish(call(a))
    i = subcommand(a)
    sub = a[i] if i is not None else ""
    build_env = dict(os.environ, CARGO_BUILD_JOBS=os.environ.get("CARGO_BUILD_JOBS", BUILD_JOBS))
    nxt = a[i + 1] if i is not None and i + 1 < len(a) else ""
    if sub in BUILD_SUBS or (sub == "nextest" and nxt in ("list", "archive")):
        finish(run("build", label, a, build_env))
    if (sub == "nextest" and nxt == "run") or sub in ("test", "t", "bench"):
        head = a[i: a.index("--", i)] if "--" in a[i:] else a[i:]
        if "--no-run" in head:
            finish(run("build", label, a, build_env))
        if not (sub in ("test", "t") and "--doc" in head):
            rc = run("build", label + " (compile)", with_no_run(a, i), build_env)
            if rc != 0:
                finish(rc)
        finish(run("test", label, a))
    finish(call(a))


if __name__ == "__main__":
    main()

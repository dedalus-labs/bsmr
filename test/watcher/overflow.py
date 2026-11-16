# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Keep the file-watcher barrier sound when the Linux inotify queue overflows."""

import ctypes
import json
import os
import select
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

ISOLATION = "overflow"
MARKER_PREFIX = ".bsmr-notify-barrier-"
NOTIFY_THREAD = "notify-rs inoti"
IN_CREATE = 0x100
PTRACE_DETACH = 17
PTRACE_SEIZE = 0x4206
PTRACE_INTERRUPT = 0x4207
WALL = 0x40000000
LIBC = ctypes.CDLL(None, use_errno=True)
LIBC.ptrace.argtypes = [ctypes.c_long, ctypes.c_long, ctypes.c_void_p, ctypes.c_void_p]
LIBC.ptrace.restype = ctypes.c_long


class NotifyThread:
    """The daemon's inotify reader thread, which ptrace pauses while the rest keeps serving."""

    def __init__(self, pid: int) -> None:
        """Find the single notify reader thread of daemon `pid`."""
        tasks = Path(f"/proc/{pid}/task")
        tids = [
            int(t.name)
            for t in tasks.iterdir()
            if (t / "comm").read_text().strip() == NOTIFY_THREAD
        ]
        assert len(tids) == 1, f"expected one notify thread in daemon {pid}, found {tids}"
        self.tid = tids[0]
        self.paused = False

    def trace(self, request: int) -> None:
        """Issue one ptrace request against the notify thread."""
        if LIBC.ptrace(request, self.tid, None, None) == -1:
            err = ctypes.get_errno()
            raise OSError(err, os.strerror(err))

    def pause(self) -> None:
        """Stop only this thread, so the kernel queues its events without delivering them."""
        self.trace(PTRACE_SEIZE)
        self.paused = True
        self.trace(PTRACE_INTERRUPT)
        tid, status = os.waitpid(self.tid, WALL)
        assert tid == self.tid, f"waited for {tid} instead of notify thread {self.tid}"
        assert os.WIFSTOPPED(status), f"notify thread did not stop: {status}"

    def resume(self) -> None:
        """Let the thread drain its queue again."""
        if self.paused:
            self.trace(PTRACE_DETACH)
            self.paused = False


class Project:
    """A fresh project driven through one bsmr binary and one isolated daemon."""

    def __init__(self, binary: Path, root: Path) -> None:
        """Drive `binary` in the project at `root`."""
        self.binary = binary
        self.root = root

    def run(self, *args: str) -> subprocess.CompletedProcess[str]:
        """Run one bounded command and retain its complete diagnostics on failure."""
        command = [self.binary, "--isolation-dir", ISOLATION, *args]
        result = subprocess.run(command, cwd=self.root, capture_output=True, text=True, timeout=180)
        assert result.returncode == 0, f"bsmr {' '.join(args)}:\n{result.stdout}{result.stderr}"
        return result

    def daemon_pid(self) -> int:
        """Read the PID the running daemon recorded for itself."""
        daemon = Path(self.run("root", "--kind", "daemon").stdout.strip())
        pid = int(json.loads((daemon / "bsmrd.info").read_text())["pid"])
        return pid

    def flood(self, directory: Path) -> None:
        """Queue more events than inotify holds, so the kernel drops the rest."""
        limit = int(Path("/proc/sys/fs/inotify/max_queued_events").read_text())
        for index in range(limit + 100):
            os.close(os.open(directory / str(index), os.O_WRONLY | os.O_CREAT, 0o600))


def await_marker(observer: int, build: subprocess.Popen[str]) -> None:
    """Block until the build creates its synchronization marker or exits."""
    exited = os.pidfd_open(build.pid)
    try:
        while True:
            ready, _, _ = select.select([observer, exited], [], [])
            assert observer in ready, f"build exited before its marker:\n{build.communicate()}"
            buf = os.read(observer, 65536)
            pos = 0
            while pos < len(buf):
                _, _, _, size = struct.unpack_from("iIII", buf, pos)
                name = buf[pos + 16 : pos + 16 + size].split(b"\0", 1)[0].decode()
                pos += 16 + size
                if name.startswith(MARKER_PREFIX):
                    return
    finally:
        os.close(exited)


def rescan_reasons(event_log: Path) -> list[str]:
    """Read why the file watcher cleared graph state during one command."""
    reasons = []
    for line in event_log.read_text().splitlines():
        if '"FileWatcher"' not in line:
            continue
        span = json.loads(line).get("Event", {}).get("data", {}).get("SpanEnd", {})
        stats = span.get("data", {}).get("FileWatcher", {}).get("stats") or {}
        if stats.get("incomplete_events_reason"):
            reasons.append(stats["incomplete_events_reason"])
    return reasons


def watch_creates(directory: Path) -> int:
    """Open an inotify descriptor that reports entries created directly in `directory`."""
    observer = LIBC.inotify_init1(os.O_CLOEXEC)
    assert observer >= 0, os.strerror(ctypes.get_errno())
    if LIBC.inotify_add_watch(observer, os.fsencode(directory), IN_CREATE) < 0:
        err = ctypes.get_errno()
        os.close(observer)
        raise OSError(err, os.strerror(err))
    return observer


def qualify(project: Project) -> None:
    """Require a build after an overflow to finish and read writes the overflow dropped."""
    source = project.root / "data.txt"
    project.run("build", "//:data")
    notify = NotifyThread(project.daemon_pid())
    event_log = project.root.parent / "overflow.json-lines"
    observer = watch_creates(project.root / "bsmr-out")
    try:
        notify.pause()
        project.flood(project.root / "bsmr-out" / "flood")
        source.write_text("after overflow\n")
        command = [project.binary, "--isolation-dir", ISOLATION, "build", "//:data"]
        command += ["--show-full-json-output", "--event-log", str(event_log)]
        build = subprocess.Popen(
            command, cwd=project.root, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True
        )
        await_marker(observer, build)
        notify.resume()
        stdout, stderr = build.communicate()
    finally:
        notify.resume()
        os.close(observer)
    assert build.returncode == 0, f"build after an inotify overflow failed:\n{stderr}"
    output = Path(json.loads(stdout)["root//:data"]) / "data.txt"
    assert output.read_text() == "after overflow\n", (
        "build reused a source read before the overflow"
    )
    reasons = rescan_reasons(event_log)
    assert reasons == ["notify rescan (no path)"], f"overflow did not clear graph state: {reasons}"


def main() -> None:
    """Overflow the daemon's inotify queue while a source edit and the next marker are in flight."""
    binary = Path(sys.argv[1]).resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="bsmr-overflow-") as temporary:
        project = Project(binary, Path(temporary).resolve() / "project")
        (project.root / "bsmr-out" / "flood").mkdir(parents=True)
        (project.root / "data.txt").write_text("before overflow\n")
        (project.root / "BUILD.bsmr").write_text('filegroup(name = "data", srcs = ["data.txt"])\n')
        project.run("init")
        try:
            qualify(project)
        finally:
            project.run("kill")
    print("ok: a build after an inotify overflow completes and reads every earlier write")


if __name__ == "__main__":
    main()

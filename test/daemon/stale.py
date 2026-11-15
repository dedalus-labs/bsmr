# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Recover from a daemon record whose PID the OS reused for an unrelated process."""

import json
import os
import select
import signal
import subprocess
import sys
import tempfile
from pathlib import Path

ISOLATION = "stale-pid"


class Project:
    """A fresh project driven through one bsmr binary and one isolated daemon."""

    def __init__(self, binary: Path, root: Path) -> None:
        self.binary = binary
        self.root = root
        self.daemon = Path.home() / ".bsmr" / "bsmrd" / root.relative_to("/") / ISOLATION

    def run(self, *args: str) -> subprocess.CompletedProcess[str]:
        """Run one bounded command and retain its complete diagnostics on failure."""
        command = [self.binary, "--isolation-dir", ISOLATION, *args]
        result = subprocess.run(command, cwd=self.root, capture_output=True, text=True, timeout=180)
        assert result.returncode == 0, f"bsmr {' '.join(args)}:\n{result.stdout}{result.stderr}"
        return result

    def daemon_pid(self) -> int:
        """Read the PID the current daemon recorded for itself."""
        return int(json.loads((self.daemon / "bsmrd.info").read_text())["pid"])

    def forge(self, pid: int) -> None:
        """Kill the daemon without cleanup, then point its surviving record at `pid`."""
        info = json.loads((self.daemon / "bsmrd.info").read_text())
        exited = os.pidfd_open(info["pid"])
        os.kill(info["pid"], signal.SIGKILL)
        select.select([exited], [], [])
        os.close(exited)
        info["pid"] = pid
        (self.daemon / "bsmrd.info").write_text(json.dumps(info))
        (self.daemon / "bsmrd.pid").write_text(str(pid))


def foreign_pid() -> int:
    """Find a live process another user owns, so signalling it fails with EPERM."""
    for entry in Path("/proc").iterdir():
        if entry.name.isdigit() and entry.stat().st_uid != os.getuid():
            return int(entry.name)
    raise AssertionError("no process owned by another user; run this test as a non-root user")


def qualify(project: Project, bystander: int, alive: subprocess.Popen[bytes] | None) -> None:
    """Require build and kill to treat a reused PID as stale and leave its process alone."""
    project.run("build", "//:data")
    project.forge(bystander)
    project.run("build", "//:data")
    assert alive is None or alive.poll() is None, "build signalled the process that reused the PID"
    assert project.daemon_pid() != bystander, "build reused the stale daemon record"
    project.forge(bystander)
    project.run("kill")
    assert alive is None or alive.poll() is None, "kill signalled the process that reused the PID"


def main() -> None:
    """Exercise a reused PID owned by this user and one owned by another user."""
    binary = Path(sys.argv[1]).resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="bsmr-stale-pid-") as temporary:
        project = Project(binary, Path(temporary).resolve())
        (project.root / "data.txt").write_text("data\n")
        (project.root / "BUILD.bsmr").write_text('filegroup(name = "data", srcs = ["data.txt"])\n')
        project.run("init")
        bystander = subprocess.Popen(["sleep", "infinity"])
        try:
            qualify(project, bystander.pid, bystander)
            qualify(project, foreign_pid(), None)
        finally:
            bystander.kill()
            bystander.wait()
            project.run("kill")
    print("ok: build and kill ignore a reused daemon PID owned by this user or another user")


if __name__ == "__main__":
    main()

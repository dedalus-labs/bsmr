# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Keep the daemon away from temporary files it never created."""

import os
import subprocess
from pathlib import Path

THREE_DAYS = 3 * 24 * 60 * 60


def run(binary: Path, root: Path, env: dict[str, str], *args: str) -> None:
    """Run the real CLI in the project and preserve diagnostics on failure."""
    result = subprocess.run(
        [binary, "--isolation-dir", "temp", *args],
        cwd=root,
        env=env,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stdout + result.stderr


def test_daemon_preserves_temp_dumps(tmp_path: Path) -> None:
    """An aged directory under the shared temp directory survives a daemon lifecycle."""
    binary = Path(os.environ["BSMR_BINARY"]).resolve(strict=True)
    root = tmp_path / "project"
    temp = tmp_path / "temp"
    home = tmp_path / "home"
    for directory in (root, temp, home):
        directory.mkdir()
    dump = temp / "bsmr-dumps" / "dice-dump-unowned"
    dump.mkdir(parents=True)
    record = dump / "nodes.gz"
    record.write_bytes(b"not created by this daemon\n")
    aged = record.stat().st_mtime - THREE_DAYS
    for path in (record, dump):
        os.utime(path, (aged, aged))
    env = os.environ | {"HOME": str(home), "TMPDIR": str(temp)}
    run(binary, root, env, "init")
    try:
        run(binary, root, env, "targets", "//...")
    finally:
        run(binary, root, env, "kill")
    assert record.read_bytes() == b"not created by this daemon\n"

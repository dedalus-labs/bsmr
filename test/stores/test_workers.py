# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Keep persistent-worker logs in the Bessemer user root."""

import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

REPOSITORY = Path(__file__).parents[2]
SOCKETS = Path("/tmp/bsmr_worker")


def build(tmp_path: Path, home: Path) -> tuple[subprocess.CompletedProcess[str], str]:
    """Build the fixture output through a real Python worker and return its build ID."""
    binary = Path(os.environ["BSMR_BINARY"]).resolve(strict=True)
    root = tmp_path / "project"
    generated = tmp_path / "generated"
    shutil.copytree(REPOSITORY / "test" / "fixtures" / "workers", root)
    generated.mkdir()
    protocol = REPOSITORY / "app" / "bsmr_worker_proto"
    outputs = [f"--python_out={generated}", f"--grpc_python_out={generated}"]
    protoc = [sys.executable, "-m", "grpc_tools.protoc", f"-I{protocol}", *outputs]
    subprocess.run([*protoc, "worker.proto"], check=True)
    env = os.environ | {"HOME": str(home)}
    command = [binary, "--isolation-dir", "workers"]
    config = ["-c", f"test.python={sys.executable}", "-c", f"test.pythonpath={generated}"]
    try:
        result = subprocess.run(
            [*command, "build", "//:output", "--show-full-json-output", *config],
            cwd=root,
            env=env,
            capture_output=True,
            text=True,
        )
    finally:
        subprocess.run([*command, "kill"], cwd=root, env=env, check=True, capture_output=True)
    trace = re.search(r"Build ID: ([0-9a-f-]+)", result.stderr)
    assert trace, result.stderr
    return result, trace[1]


def test_worker_logs(tmp_path: Path) -> None:
    """A real worker's output log lives below HOME, not beside its shared socket."""
    home = tmp_path / "home"
    home.mkdir()
    result, trace = build(tmp_path, home)
    assert result.returncode == 0, result.stdout + result.stderr
    output = Path(json.loads(result.stdout)["root//:output"])
    assert output.read_text() == "written by worker\n"
    logs = list((home / ".bsmr" / "workers").glob(f"{trace}-*/stderr"))
    assert len(logs) == 1
    assert "worker wrote output" in logs[0].read_text()
    assert not list(SOCKETS.glob(f"{trace}-*/stderr"))


def test_unusable_log_root(tmp_path: Path) -> None:
    """A worker that cannot create its log directory leaves no socket directory behind."""
    home = tmp_path / "home"
    (home / ".bsmr").mkdir(parents=True)
    (home / ".bsmr" / "workers").write_text("not a directory\n")
    result, trace = build(tmp_path, home)
    assert result.returncode != 0, result.stdout
    assert not list(SOCKETS.glob(f"{trace}-*"))

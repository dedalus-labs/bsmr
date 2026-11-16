# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Keep verified downloads and reusable command results in the Bessemer user root."""

import hashlib
import json
import os
import shutil
import subprocess
from collections.abc import Iterator
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from threading import Thread

import pytest


class Project:
    """Drive the real binary with independent home and platform-cache directories."""

    def __init__(self, root: Path) -> None:
        """Copy static build inputs and select an isolated user cache namespace."""
        self.binary = Path(os.environ["BSMR_BINARY"]).resolve(strict=True)
        self.root = root / "project"
        self.home = root / "home"
        self.platform_cache = root / "platform-cache"
        self.home.mkdir()
        shutil.copytree(Path(__file__).parents[1] / "fixtures" / "user-stores", self.root)
        self.env = os.environ | {
            "HOME": str(self.home),
            "XDG_CACHE_HOME": str(self.platform_cache),
        }
        self.env.pop("BSMR_LOCAL_CACHE_DIR", None)
        self.env.pop("BSMR_HTTP_CACHE_DIR", None)

    def run(self, *args: str) -> subprocess.CompletedProcess[str]:
        """Run the selected isolation and preserve diagnostics on failure."""
        command = [str(self.binary), "--isolation-dir", "user-stores", *args]
        result = subprocess.run(
            command, cwd=self.root, env=self.env, capture_output=True, text=True
        )
        assert result.returncode == 0, result.stdout + result.stderr
        return result

    def build(self, target: str, *config: str) -> Path:
        """Return the physical artifact reported by a successful real build."""
        result = self.run("build", target, "--show-full-json-output", *config)
        outputs: dict[str, str] = json.loads(result.stdout)
        output = Path(outputs[target])
        return output


@pytest.fixture
def project(tmp_path: Path) -> Iterator[Project]:
    """Stop the test's daemon even when an expected cache path is missing."""
    result = Project(root=tmp_path)
    try:
        yield result
    finally:
        result.run("kill")


@pytest.fixture
def download_server(project: Project) -> Iterator[ThreadingHTTPServer]:
    """Serve immutable fixture bytes through a real local HTTP listener."""
    handler = partial(SimpleHTTPRequestHandler, directory=str(project.root))
    with ThreadingHTTPServer(("127.0.0.1", 0), handler) as server:
        thread = Thread(target=server.serve_forever)
        thread.start()
        try:
            yield server
        finally:
            server.shutdown()
            thread.join()


def test_http_store(project: Project, download_server: ThreadingHTTPServer) -> None:
    """A verified HTTP response lives below HOME, independent of XDG cache placement."""
    payload = (project.root / "payload.txt").read_bytes()
    digest = hashlib.sha256(payload).hexdigest()
    url = f"http://127.0.0.1:{download_server.server_port}/payload.txt"
    config = (
        "-c",
        f"test.url={url}",
        "-c",
        f"test.sha256={digest}",
        "-c",
        f"test.size_bytes={len(payload)}",
    )
    output = project.build("root//:download", *config)
    assert output.read_bytes() == payload
    cache = project.home / ".bsmr" / "http-v1" / "sha256" / digest[:2] / digest
    assert cache.read_bytes() == payload
    assert not (project.platform_cache / "bsmr" / "http-v1").exists()
    download_server.shutdown()
    download_server.server_close()
    project.run("clean")
    restored = project.build("root//:download", *config)
    assert restored.read_bytes() == payload


def test_action_store(project: Project) -> None:
    """An executed copy is reusable after clean without deleting the shared store."""
    payload = (project.root / "payload.txt").read_bytes()
    output = project.build("root//:copy")
    assert output.read_bytes() == payload
    cache = project.home / ".bsmr" / "action-v1"
    assert cache.is_dir()
    entries = sorted(path.relative_to(cache) for path in cache.rglob("*") if path.is_file())
    assert entries
    assert not (project.platform_cache / "bsmr" / "action-v1").exists()
    project.run("clean")
    restored = project.build("root//:copy")
    assert restored.read_bytes() == payload
    assert all((cache / path).is_file() for path in entries)
    event = json.loads(project.run("log", "what-ran", "--format", "json").stdout)
    assert event["reproducer"]["executor"] == "Cache"

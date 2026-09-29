# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Build, edit, and restore two real executables through experimental snapshots."""

import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib

binary = str(Path(sys.argv[1]).resolve())
with tempfile.TemporaryDirectory(prefix="bsmr-snapshot-") as temporary:
    root = Path(temporary)
    env = dict(os.environ, BSMR_LOCAL_CACHE_DIR=str(root / "cache"))

    def run(*args: str, success: bool = True) -> subprocess.CompletedProcess[str]:
        """Run the actual CLI and retain its diagnostics on failure."""
        result = subprocess.run([binary, *args], cwd=root, env=env, text=True, capture_output=True, timeout=180)
        if success:
            assert result.returncode == 0, result.stderr
        return result

    def capture(name: str) -> tuple[bytes, dict[str, str]]:
        """Verify emitted identities against materialized executable bytes."""
        result = run("build", "//api:bin", "//worker:bin", "-c", "go.link_mode=internal", "--snapshot", name)
        data = (root / name).read_bytes()
        lock = tomllib.loads(data.decode())
        assert lock["schema"] == "bsmr.dependency-lock.experimental.v0"
        assert lock["digest_algorithm"] == "sha256"
        digests = {}
        for target, configurations in lock["targets"].items():
            assert len(configurations) == 1
            artifacts = next(iter(configurations.values()))
            assert len(artifacts) == 1
            for path, artifact in artifacts.items():
                output = root / path
                digest, size = artifact["digest"].split(":")
                assert output.stat().st_size == int(size)
                assert hashlib.sha256(output.read_bytes()).hexdigest() == digest
                assert artifact["is_exec"]
                assert subprocess.check_output([str(output)], text=True).strip() in ("api-v1", "api-v2", "worker")
                digests[target] = digest
        trace = re.search(r"Build ID: ([a-f0-9-]+)", result.stderr)[1]
        log = run("log", "what-ran", "--trace-id", trace, "--format", "json", "--filter-category", "go_.*", "--no-remote")
        local = [json.loads(line) for line in log.stdout.splitlines() if json.loads(line)["reproducer"]["executor"] == "Local"]
        if name == "v2.lock":
            assert not any("worker" in action["identity"] for action in local)
        return data, digests

    try:
        (root / "go.mod").write_text("module example.com/snapshot\n\ngo 1.26.0\n")
        (root / "bsmr.component.toml").write_text("schema=1\nname='example'\n")
        for name, message in (("api", "api-v1"), ("worker", "worker")):
            (root / name).mkdir()
            (root / name / "main.go").write_text(f'package main\nimport "fmt"\nfunc main() {{ fmt.Println("{message}") }}\n')
        assert run("build", "--snapshot", "", success=False).returncode != 0
        assert run("build", "--snapshot", "same.lock", "--build-report", "same.lock", success=False).returncode != 0
        assert not (root / "same.lock").exists()
        run("init")
        run("go", "toolchain", "--version", "1.26.1")
        run("go", "sync")
        first, before = capture("v1.lock")
        assert capture("same.lock")[0] == first
        source = root / "api/main.go"
        original = source.read_text()
        source.write_text(original.replace("api-v1", "api-v2"))
        _, after = capture("v2.lock")
        assert before["root//worker:bin"] == after["root//worker:bin"]
        assert before["root//api:bin"] != after["root//api:bin"]
        source.write_text(original)
        assert capture("restored.lock")[0] == first
        assert run("build", "//api:bin", "--snapshot", "v1.lock", success=False).returncode != 0
        assert (root / "v1.lock").read_bytes() == first
        source.write_text("invalid go\n")
        assert run("build", "//api:bin", "--snapshot", "failed.lock", success=False).returncode != 0
        assert not (root / "failed.lock").exists()
        print("PASS: exact bytes, unchanged identity, isolated edit, cached restore, immutable output, failed build")
    finally:
        run("kill")

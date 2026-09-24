# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Verifies declared-input execution and cache identity against a real namespace backend.

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import tarfile
import tempfile
import time

PROGRAM = """\
from pathlib import Path
import os, socket, subprocess, sys, time

mode, source, output, outside, port, generated, tree = sys.argv[1:]
source, output = Path(source), Path(output)
output.mkdir()
assert Path(generated).read_text() == "generated"
assert (Path(tree) / "link").is_symlink()
assert (Path(tree) / "link").read_text() == (source / "value").read_text()
if mode != "directory":
    try:
        Path(generated).write_text("changed")
    except OSError:
        pass
    else:
        raise AssertionError("generated input was writable")
    try:
        (Path(tree) / "value").write_text("changed")
    except OSError:
        pass
    else:
        raise AssertionError("generated input tree was writable")
if mode in ["directory", "runtime"]:
    (output / "result").write_text(
        os.getcwd() if mode == "directory" else Path("/bsmr-runtime-value").read_text()
    )
    sys.exit(0)
if mode in ["exit", "timeout", "cancel"]:
    child = "import os,sys,time; os.setsid(); stream=open(sys.argv[1],'w'); [(stream.seek(0),stream.write(str(i)),stream.flush(),time.sleep(.05)) for i in range(100)]"
    subprocess.Popen(
        ["/usr/bin/python3", "-c", child, str(output / "heartbeat")],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    while not (output / "heartbeat").exists():
        time.sleep(0.01)
    if mode != "exit":
        time.sleep(30)
    sys.exit(0)
if mode == "escape":
    (output / "escape").symlink_to(outside)
    sys.exit(0)
assert os.getcwd() == "/workspace"
assert Path("/proc/self/exe").resolve() == Path("/usr/bin/python3")
processes = {path.name for path in Path("/proc").iterdir() if path.name.isdecimal()}
assert processes == {"1", str(os.getpid())}
assert Path("/proc/1/root/workspace").is_dir()
assert not (Path("/proc/1/root") / outside.lstrip("/")).exists()
assert os.statvfs("/proc").f_flag & os.ST_RDONLY
assert os.environ["DECLARED"] == "visible"
assert "NAMESPACE_AMBIENT_SECRET" not in os.environ
assert (source / "value").read_text() == (source / "link").read_text()
assert (
    subprocess.check_output(["/bin/cat", str(source / "value")]).decode()
    == (source / "value").read_text()
)
try:
    (source / "value").write_text("changed")
except OSError:
    pass
else:
    raise AssertionError("input was writable")
try:
    Path(outside).read_bytes()
except OSError:
    pass
else:
    raise AssertionError("undeclared file readable")
assert subprocess.run(["/bin/cat", outside], capture_output=True).returncode != 0
try:
    socket.create_connection(("127.0.0.1", int(port)), timeout=0.2)
except OSError:
    pass
else:
    raise AssertionError("host network reachable")
scratch = Path(os.environ["BSMR_SCRATCH_PATH"])
(scratch / "probe").write_text("scratch")
(output / "result").write_text((source / "value").read_text())
(output.parent / "undeclared").write_text("discard")
"""
RULES = '''\
def _impl(ctx):
    """Run one bounded probe with explicit source and output artifacts."""
    out = ctx.actions.declare_output("out", dir = True)
    generated = ctx.actions.write("generated", "generated")
    if ctx.attrs.mode == "input_symlink":
        generated = ctx.actions.symlink_file("generated-link", generated)
    tree = ctx.actions.copy_dir("tree", ctx.attrs.source)
    ctx.actions.run(cmd_args("/usr/bin/python3", ctx.attrs.program, ctx.attrs.mode, ctx.attrs.source, out.as_output(), ctx.attrs.outside, ctx.attrs.port, generated, tree), category = "namespace_conformance", allow_cache_upload = True, env = {"DECLARED": "visible"}, timeout_seconds = ctx.attrs.timeout)
    return [DefaultInfo(default_output = out)]
probe = rule(impl = _impl, attrs = {"program": attrs.source(), "source": attrs.source(allow_directory = True), "mode": attrs.string(), "outside": attrs.string(), "port": attrs.string(), "timeout": attrs.int(default = 20)})

def _test(ctx):
    """Tests use explicit runtime inputs rather than the host's environment."""
    return [DefaultInfo(), ExternalRunnerTestInfo(type = "simple", command = [cmd_args("/usr/bin/python3", ctx.attrs.program)], env = {"DECLARED": "visible"}, run_from_project_root = True, use_project_relative_paths = True)]

probe_test = rule(impl = _test, attrs = {"program": attrs.source()})
'''


def main() -> None:
    """Run real actions using an explicit BSMR binary and pinned runtime manifest."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("runtime", type=Path)
    args = parser.parse_args()
    binary = str(args.binary.resolve(strict=True))
    manifest = args.runtime.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="bsmr-namespace-test-") as temporary:
        base = Path(temporary).resolve()
        runtime = base / "runtime"
        runtime.mkdir()
        pins = json.loads(manifest.read_text())
        assert set(pins) == {"bubblewrap", "rootfs"}
        for pin in pins.values():
            assert Path(pin["path"]).name == pin["path"]
            shutil.copyfile(manifest.parent / pin["path"], runtime / pin["path"])
        manifest = runtime / "runtime.json"
        manifest.write_text(json.dumps(pins))
        root = base / "first"
        root.mkdir()
        checkouts = [root]
        env = {
            **os.environ,
            "BSMR_LOCAL_CACHE_DIR": str(base / "cache"),
            "NAMESPACE_AMBIENT_SECRET": "outside",
        }

        def run(*arguments: str, cwd: Path = root) -> subprocess.CompletedProcess[str]:
            """Bound one BSMR invocation and retain diagnostics for failed assertions."""
            return subprocess.run(
                [binary, *arguments],
                cwd=cwd,
                env=env,
                capture_output=True,
                text=True,
                timeout=30,
            )

        def build(
            target: str, sandbox: bool = True, cwd: Path = root
        ) -> subprocess.CompletedProcess[str]:
            """Execute the requested target with an explicit process boundary."""
            return run(
                "build",
                "//:" + target,
                "--console",
                "simple",
                "--show-full-json-output",
                *(["--sandbox"] if sandbox else []),
                cwd=cwd,
            )

        def output(result: subprocess.CompletedProcess[str]) -> Path:
            """Require successful execution before returning its declared output directory."""
            assert result.returncode == 0, result.stderr
            return Path(next(iter(json.loads(result.stdout).values())))

        def runtime_value(value: str) -> None:
            """Change verified runtime bytes without changing the manifest location."""
            archive = runtime / pins["rootfs"]["path"]
            replacement = archive.with_suffix(".next")
            with (
                tarfile.open(archive) as source,
                tarfile.open(replacement, "w") as target,
            ):
                for member in source:
                    if member.name != "bsmr-runtime-value":
                        target.addfile(
                            member,
                            source.extractfile(member) if member.isfile() else None,
                        )
                data = value.encode()
                member = tarfile.TarInfo("bsmr-runtime-value")
                member.size = len(data)
                member.mode = 0o644
                target.addfile(member, io.BytesIO(data))
            replacement.replace(archive)
            with archive.open("rb") as stream:
                pins["rootfs"]["sha256"] = hashlib.file_digest(
                    stream, "sha256"
                ).hexdigest()
            manifest.write_text(json.dumps(pins))

        initialized = run("init")
        assert initialized.returncode == 0, initialized.stderr
        (root / ".bsmr.local").write_text(
            f"[sandbox]\nbackend = namespace\nruntime = {manifest}\n"
        )
        (root / "input").mkdir()
        (root / "input/value").write_text("7")
        (root / "input/link").symlink_to("value")
        outside = base / "outside"
        outside.write_text("outside")
        assert subprocess.check_output(["/bin/cat", str(outside)]) == b"outside"
        (root / "probe.py").write_text(PROGRAM)
        (root / "test.py").write_text(
            'import os\n'
            'assert os.environ["HOME"] == "/tmp"\n'
            'assert os.environ["DECLARED"] == "visible"\n'
            'assert "NAMESPACE_AMBIENT_SECRET" not in os.environ\n'
        )
        (root / "rule.bzl").write_text(RULES)
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            listener.listen()
            port = str(listener.getsockname()[1])
            with socket.create_connection(("127.0.0.1", int(port)), timeout=1):
                pass
            modes = [
                "directory",
                "runtime",
                "declared",
                "exit",
                "timeout",
                "cancel",
                "escape",
                "input_symlink",
            ]
            definitions = [
                f'probe(name={json.dumps(mode)}, program="probe.py", source="input", mode={json.dumps(mode)}, outside={json.dumps(str(outside))}, port={json.dumps(port)}, timeout={1 if mode == "timeout" else 20})'
                for mode in modes
            ]
            (root / "BUILD.bsmr").write_text(
                'load(":rule.bzl", "probe", "probe_test")\n'
                'probe_test(name="test", program="test.py")\n'
                + "\n".join(definitions) + "\n"
            )
            try:
                tested = run("test", "//:test", "--sandbox", "--console", "simple")
                assert tested.returncode == 0, tested.stderr
                assert "NO TESTS RAN" not in tested.stderr, tested.stderr
                print("  ok  isolated tests use explicit environment", flush=True)
                assert (
                    output(build("directory", False)) / "result"
                ).read_text() == str(root)
                assert (
                    output(build("directory")) / "result"
                ).read_text() == "/workspace"
                print("  ok  host-to-namespace invalidation", flush=True)
                for value in ["7", "9"]:
                    runtime_value(value)
                    assert (output(build("runtime")) / "result").read_text() == value
                print("  ok  runtime-content invalidation", flush=True)

                assert (
                    output(build("directory")) / "result"
                ).read_text() == "/workspace"
                second = base / "second"
                shutil.copytree(
                    root,
                    second,
                    symlinks=True,
                    ignore=shutil.ignore_patterns("bsmr-out"),
                )
                checkouts.append(second)
                reused = build("directory", cwd=second)
                assert (output(reused) / "result").read_text() == "/workspace"
                assert "cached: 1" in reused.stderr, reused.stderr
                print("  ok  cache reuse across checkouts", flush=True)

                for value in ["7", "9"]:
                    (root / "input/value").write_text(value)
                    directory = output(build("declared"))
                    assert (directory / "result").read_text() == value
                    assert not (directory.parent / "undeclared").exists()
                print(
                    "  ok  declared inputs, explicit environment, denied host reads/network",
                    flush=True,
                )
                heartbeat = output(build("exit")) / "heartbeat"
                before = heartbeat.read_text()
                time.sleep(0.2)
                assert heartbeat.read_text() == before, (
                    "detached child survived success"
                )
                print("  ok  descendants exit with action", flush=True)

                for mode in ["timeout", "cancel"]:
                    command = [
                        binary,
                        "build",
                        "//:" + mode,
                        "--sandbox",
                        "--console",
                        "simple",
                    ]
                    with subprocess.Popen(
                        command,
                        cwd=root,
                        env=env,
                        stdout=subprocess.PIPE,
                        stderr=subprocess.PIPE,
                        text=True,
                    ) as process:
                        deadline = time.monotonic() + 10
                        heartbeats: list[Path] = []
                        while time.monotonic() < deadline and process.poll() is None:
                            heartbeats = list(
                                root.glob(
                                    "bsmr-out/**/.bsmr-namespace-*/outputs/**/heartbeat"
                                )
                            )
                            if heartbeats:
                                break
                            time.sleep(0.02)
                        assert len(heartbeats) == 1, "detached child did not start"
                        with heartbeats[0].open("rb") as stream:
                            if mode == "cancel":
                                process.send_signal(signal.SIGINT)
                            _, stderr = process.communicate(timeout=10)
                            assert process.returncode != 0, stderr
                            if mode == "timeout":
                                assert "timed out" in stderr.lower(), stderr
                            before = os.pread(stream.fileno(), 32, 0)
                            time.sleep(0.2)
                            assert os.pread(stream.fileno(), 32, 0) == before, (
                                f"detached child survived {mode}"
                            )
                    print(f"  ok  descendants exit on {mode}", flush=True)
                rejected = build("escape")
                assert rejected.returncode != 0 and "escapes" in rejected.stderr, (
                    rejected.stderr
                )
                print("  ok  output symlink rejection", flush=True)
                rejected = build("input_symlink")
                assert (
                    rejected.returncode != 0
                    and "shares a writable output parent" in rejected.stderr
                ), rejected.stderr
                print("  ok  unsupported input symlink rejection", flush=True)
            finally:
                for checkout in checkouts:
                    run("kill", cwd=checkout)


if __name__ == "__main__":
    main()

# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Executes native Python actions with exact tools and isolated mutable state.

"""Execute native Python actions with exact tools and isolated mutable state."""

from __future__ import annotations

import argparse
import base64
import csv
import hashlib
import os
import shutil
import subprocess
from pathlib import Path
from typing import Final

_SOURCE_DATE_EPOCH: Final = "315532800"


def _arguments() -> argparse.Namespace:
    """Parse the runner's closed command-line contract."""
    parser = argparse.ArgumentParser(allow_abbrev=False)
    parser.add_argument(
        "mode", choices=("environment", "ruff", "ty", "wheel", "wheel-environment")
    )
    parser.add_argument("--build-environment", type=Path)
    parser.add_argument("--environment", type=Path)
    parser.add_argument("--lock", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--project-root")
    parser.add_argument("--python", type=Path, required=True)
    parser.add_argument("--ruff", type=Path)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--ty", type=Path)
    parser.add_argument("--uv", type=Path)
    parser.add_argument("--vcs", type=Path)
    parser.add_argument("--wheel-dir", action="append", default=[], type=Path)
    return parser.parse_args()


def _required(value: object | None, name: str) -> object:
    """Return a mode-specific argument or fail with its exact missing name."""
    if value is None:
        raise ValueError(f"{name} is required for this action")
    return value


def _state(output: Path) -> tuple[Path, dict[str, str]]:
    """Create isolated process state outside the content-addressed output."""
    scratch_value = os.environ.get("BUCK_SCRATCH_PATH")
    if not scratch_value:
        raise RuntimeError("BSMR did not provide BUCK_SCRATCH_PATH")
    scratch = Path(scratch_value).resolve() / "python"
    output = output.resolve()
    if scratch == output or output in scratch.parents or scratch in output.parents:
        raise RuntimeError("Python mutable state and cached output must be disjoint")
    shutil.rmtree(scratch, ignore_errors=True)
    scratch.mkdir(parents=True)
    environment = {
        "HOME": str(scratch / "home"),
        "LANG": "C.UTF-8",
        "NO_COLOR": "1",
        "PATH": os.defpath,
        "SOURCE_DATE_EPOCH": _SOURCE_DATE_EPOCH,
        "UV_CACHE_DIR": str(scratch / "uv-cache"),
        "UV_NO_CONFIG": "1",
        "UV_PYTHON_DOWNLOADS": "never",
        "XDG_CACHE_HOME": str(scratch / "xdg-cache"),
        "XDG_CONFIG_HOME": str(scratch / "xdg-config"),
    }
    for path in ("HOME", "UV_CACHE_DIR", "XDG_CACHE_HOME", "XDG_CONFIG_HOME"):
        Path(environment[path]).mkdir()
    return scratch, environment


def _run(
    command: list[str], environment: dict[str, str], cwd: Path | None = None
) -> None:
    """Run one exact command and propagate its nonzero status."""
    completed = subprocess.run(command, check=False, cwd=cwd, env=environment)
    if completed.returncode != 0:
        raise SystemExit(completed.returncode)


def _record_digest(data: bytes) -> str:
    """Return a wheel RECORD-compatible SHA-256 digest."""
    digest = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=")
    return f"sha256={digest.decode('ascii')}"


def _normalize_entry_points(packages: Path) -> None:
    """Remove output-path shebangs and update their owning wheel RECORD rows."""
    scripts = packages / "bin"
    replacements: dict[str, tuple[str, str]] = {}
    if scripts.is_dir():
        for script in scripts.iterdir():
            if not script.is_file():
                continue
            data = script.read_bytes()
            if not data.startswith(b"#!"):
                continue
            _, separator, body = data.partition(b"\n")
            if not separator:
                raise RuntimeError(
                    f"entry point '{script}' has an unterminated shebang"
                )
            normalized = b"#!/usr/bin/env python3\n" + body
            script.write_bytes(normalized)
            relative = script.relative_to(packages).as_posix()
            replacements[relative] = (_record_digest(normalized), str(len(normalized)))
    for record in packages.glob("*.dist-info/RECORD"):
        rows = list(csv.reader(record.read_text(encoding="utf-8").splitlines()))
        changed = False
        for row in rows:
            if row and row[0] in replacements:
                row[1], row[2] = replacements[row[0]]
                changed = True
        if changed:
            with record.open("w", encoding="utf-8", newline="") as output:
                csv.writer(output, lineterminator="\n").writerows(rows)
    (packages / ".lock").unlink(missing_ok=True)


def _validate_environment(packages: Path) -> None:
    """Reject ambiguous filesystem objects before committing a CAS tree."""
    for path in packages.rglob("*"):
        if path.is_symlink() or not (path.is_file() or path.is_dir()):
            raise RuntimeError(
                f"Python environment contains unsupported entry '{path}'"
            )


def _activate_environment(root: Path, process_environment: dict[str, str]) -> Path:
    """Expose one exact CAS-backed package tree to a Python process."""
    environment = root.resolve()
    if not environment.is_dir():
        raise RuntimeError(f"Python environment '{root}' is not a directory")
    process_environment["PATH"] = os.pathsep.join(
        (str(environment / "bin"), process_environment["PATH"])
    )
    process_environment["PYTHONPATH"] = str(environment)
    return environment


def _environment(
    args: argparse.Namespace, process_environment: dict[str, str], scratch: Path
) -> None:
    """Install one complete PEP 751 set against its explicit build closure."""
    lock = _required(args.lock, "--lock")
    uv = _required(args.uv, "--uv")
    packages = args.output.resolve()
    packages.mkdir()
    build_flags = ["--no-build"]
    if args.build_environment is not None:
        _activate_environment(args.build_environment, process_environment)
        build_flags = ["--no-build-isolation"]
    _run(
        [
            str(uv),
            "pip",
            "sync",
            str(lock),
            "--target",
            str(packages),
            "--python",
            str(args.python),
            "--no-python-downloads",
            *build_flags,
            "--strict",
            "--preview-features",
            "pylock",
            "--color",
            "never",
            "--no-progress",
        ],
        process_environment,
    )
    _normalize_entry_points(packages)
    _validate_environment(packages)


def _wheel_environment(
    args: argparse.Namespace, process_environment: dict[str, str], scratch: Path
) -> None:
    """Install exact first-party wheels as a separately cacheable runtime layer."""
    uv = _required(args.uv, "--uv")
    packages = args.output.resolve()
    packages.mkdir()
    wheels = []
    for directory in args.wheel_dir:
        candidates = list(directory.resolve().glob("*.whl"))
        if len(candidates) != 1:
            raise RuntimeError(
                f"first-party wheel directory '{directory}' contains {len(candidates)} wheels"
            )
        wheels.extend(candidates)
    if not wheels:
        raise RuntimeError("Python wheel environment requires at least one wheel")
    _run(
        [
            str(uv),
            "pip",
            "install",
            *map(str, wheels),
            "--target",
            str(packages),
            "--python",
            str(args.python),
            "--no-build",
            "--no-deps",
            "--no-index",
            "--strict",
            "--color",
            "never",
            "--no-progress",
        ],
        process_environment,
    )
    _normalize_entry_points(packages)
    _validate_environment(packages)


def _project(
    args: argparse.Namespace, process_environment: dict[str, str], scratch: Path
) -> None:
    """Execute one first-party build, lint, or typecheck action."""
    source = Path(_required(args.source, "--source")).resolve()
    project_root = _required(args.project_root, "--project-root")
    project = (source / str(project_root)).resolve()
    output = args.output.resolve()
    python = args.python.resolve()
    process_environment["GIT_CEILING_DIRECTORIES"] = str(source.parent)
    if args.vcs is not None:
        git_directory = scratch / "git"
        git_directory.mkdir()
        for child in args.vcs.resolve().iterdir():
            destination = git_directory / child.name
            if child.is_dir():
                destination.symlink_to(child, target_is_directory=True)
            else:
                shutil.copyfile(child, destination)
        (git_directory / "config").write_text(
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n",
            encoding="utf-8",
        )
        process_environment["GIT_CONFIG_GLOBAL"] = os.devnull
        process_environment["GIT_CONFIG_NOSYSTEM"] = "1"
        process_environment["GIT_DIR"] = str(git_directory)
        process_environment["GIT_INDEX_FILE"] = str(git_directory / "index")
        process_environment["GIT_WORK_TREE"] = str(source)
        _run(["git", "read-tree", "HEAD"], process_environment, cwd=source)
    if args.mode == "wheel":
        del process_environment["UV_NO_CONFIG"]
        uv = Path(_required(args.uv, "--uv")).resolve()
        environment = _activate_environment(
            Path(_required(args.environment, "--environment")),
            process_environment,
        )
        output.mkdir()
        command = [
            str(uv),
            "build",
            "--wheel",
            "--no-build-isolation",
            "--no-build-logs",
            "--no-create-gitignore",
            "--out-dir",
            str(output),
            "--python",
            str(python),
            "--no-python-downloads",
            "--color",
            "never",
            "--no-progress",
            ".",
        ]
    elif args.mode == "ruff":
        ruff = Path(_required(args.ruff, "--ruff")).resolve()
        command = [str(ruff), "check", "--no-cache", "--output-format", "concise", "."]
    else:
        environment = _activate_environment(
            Path(_required(args.environment, "--environment")),
            process_environment,
        )
        ty = Path(_required(args.ty, "--ty")).resolve()
        command = [
            str(ty),
            "check",
            "--python",
            str(python),
            "--extra-search-path",
            str(environment),
            "--no-progress",
            "--color",
            "never",
            ".",
        ]
    _run(command, process_environment, cwd=project)
    if args.mode != "wheel":
        output.write_text("ok\n", encoding="utf-8")


def main() -> None:
    """Dispatch one validated native Python action."""
    args = _arguments()
    if args.output.exists():
        raise FileExistsError(f"action output '{args.output}' already exists")
    scratch, process_environment = _state(args.output)
    if args.mode == "environment":
        _environment(args, process_environment, scratch)
    elif args.mode == "wheel-environment":
        _wheel_environment(args, process_environment, scratch)
    else:
        _project(args, process_environment, scratch)


if __name__ == "__main__":
    main()

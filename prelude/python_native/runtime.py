# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Runs Python entry points and tests from declared source and environment trees.

"""Run Python entry points and tests from declared source and environment trees."""

from __future__ import annotations

import argparse
import importlib
import os
import sys
import tempfile
from pathlib import Path
from typing import Callable


def _arguments() -> argparse.Namespace:
    """Parse the runtime's closed command-line contract."""
    parser = argparse.ArgumentParser(allow_abbrev=False)
    parser.add_argument("mode", choices=("entry", "test"))
    parser.add_argument("--entry")
    parser.add_argument("--environment", action="append", type=Path, required=True)
    parser.add_argument("--project-root", required=True)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("arguments", nargs=argparse.REMAINDER)
    return parser.parse_args()


def _project_roots(source: Path) -> list[Path]:
    """Return every declared workspace member as a deterministic import root."""
    return sorted({manifest.parent for manifest in source.rglob("pyproject.toml")})


def _environment_root(path: Path) -> Path:
    """Return one materialized CAS tree or reject an invalid artifact shape."""
    root = path.resolve()
    if not root.is_dir():
        raise RuntimeError(f"Python environment '{path}' is not a directory")
    return root


def _child_interpreter(scratch: Path, declared: list[str]) -> Path:
    """Create a minimal venv whose children inherit only declared import roots."""
    interpreter = Path(sys.executable).resolve()
    runtime_bin = scratch / "bin"
    runtime_bin.mkdir()
    for name in ("python", "python3"):
        (runtime_bin / name).symlink_to(interpreter)
    site_packages = (
        scratch
        / "lib"
        / f"python{sys.version_info.major}.{sys.version_info.minor}"
        / "site-packages"
    )
    site_packages.mkdir(parents=True)
    (site_packages / "bsmr.pth").write_text(
        "".join(f"{path}\n" for path in declared), encoding="utf-8"
    )
    (scratch / "pyvenv.cfg").write_text(
        f"home = {interpreter.parent}\n"
        "include-system-site-packages = false\n"
        f"version = {sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}\n"
        f"executable = {interpreter}\n",
        encoding="utf-8",
    )
    return runtime_bin.resolve()


def _bootstrap(args: argparse.Namespace, scratch: Path) -> Path:
    """Install only declared import roots and deterministic process state."""
    if not scratch.is_dir():
        raise ValueError(f"runtime scratch directory '{scratch}' does not exist")
    source = args.source.resolve()
    project = (source / args.project_root).resolve()
    if project != source and source not in project.parents:
        raise ValueError(f"project root '{args.project_root}' escapes the source tree")
    environments = [_environment_root(environment) for environment in args.environment]
    home = scratch / "home"
    home.mkdir()
    declared = [str(root) for root in _project_roots(source)] + [
        str(environment) for environment in environments
    ]
    runtime_bin = _child_interpreter(scratch, declared)
    sys.path[:] = declared + [
        path for path in sys.path if path and path not in declared
    ]
    os.environ.update(
        {
            "HOME": str(home),
            "NO_COLOR": "1",
            "PATH": os.pathsep.join(
                [
                    str(runtime_bin),
                    *(str(environment / "bin") for environment in environments),
                    os.defpath,
                ]
            ),
            "PYTHONDONTWRITEBYTECODE": "1",
            "PYTHONNOUSERSITE": "1",
            "PYTHONPATH": os.pathsep.join(declared),
        }
    )
    os.chdir(project)
    return project


def _entry(specification: str) -> Callable[[], object]:
    """Resolve one standard ``module:object`` console-script reference."""
    module_name, separator, object_path = specification.partition(":")
    if not separator or not module_name or not object_path:
        raise ValueError(f"entry point '{specification}' must use module:object syntax")
    value: object = importlib.import_module(module_name)
    for component in object_path.split("."):
        value = getattr(value, component)
    if not callable(value):
        raise TypeError(f"entry point '{specification}' is not callable")
    return value


def main() -> int:
    """Execute one entry point or pytest session and return its exact status."""
    args = _arguments()
    with tempfile.TemporaryDirectory(prefix="bsmr-python-runtime-") as temporary:
        _bootstrap(args, Path(temporary))
        sys.argv[:] = [sys.argv[0], *args.arguments]
        if args.mode == "entry":
            result = _entry(args.entry or "")()
            return result if isinstance(result, int) else 0
        pytest = importlib.import_module("pytest")
        return int(pytest.main())


if __name__ == "__main__":
    raise SystemExit(main())

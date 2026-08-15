# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Executes one declared PEP 517 wheel hook for the semantics-matched Bazel control.

"""Execute one declared PEP 517 wheel hook in an exact Bazel build environment."""

from __future__ import annotations

import argparse
import base64
import csv
import hashlib
import importlib
import os
import shlex
import shutil
import sys
import tempfile
import zipfile
from email.parser import Parser
from pathlib import Path
from typing import Protocol, cast

import tomllib


class BuildWheel(Protocol):
    """The PEP 517 wheel-hook shape consumed by this benchmark."""

    def __call__(
        self,
        wheel_directory: str,
        config_settings: dict[str, str | list[str]] | None = None,
        metadata_directory: str | None = None,
    ) -> str:
        """Build and return one wheel filename."""


class ArgumentParser(argparse.ArgumentParser):
    """Parses Bazel's shell-quoted multiline parameter files."""

    def convert_arg_line_to_args(self, arg_line: str) -> list[str]:
        """Decode one Bazel parameter-file line."""
        return shlex.split(arg_line)


def _arguments() -> argparse.Namespace:
    """Parse the closed benchmark action contract."""
    parser = ArgumentParser(allow_abbrev=False, fromfile_prefix_chars="@")
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--source", action="append", required=True, type=Path)
    return parser.parse_args()


def _copy_project(sources: list[Path], project: Path) -> None:
    """Copy declared inputs so an arbitrary backend cannot mutate the workspace."""
    for source in sources:
        relative = Path(os.path.normpath(source))
        if relative.is_absolute() or ".." in relative.parts:
            raise RuntimeError(f"non-project PEP 517 input {source!r}")
        destination = project / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(source, destination)
    timestamp = int(os.environ["SOURCE_DATE_EPOCH"])
    for path in [project, *project.rglob("*")]:
        os.utime(path, (timestamp, timestamp), follow_symlinks=False)


def _backend(project: Path) -> BuildWheel:
    """Load the exact backend named by the copied project metadata."""
    pyproject = tomllib.loads((project / "pyproject.toml").read_text(encoding="utf-8"))
    build_system = pyproject.get("build-system")
    if not isinstance(build_system, dict):
        raise TypeError("pyproject.toml has no [build-system]")
    name = build_system.get("build-backend")
    if not isinstance(name, str) or not name:
        raise RuntimeError("[build-system].build-backend must be a nonempty string")
    backend_paths = build_system.get("backend-path", [])
    if not isinstance(backend_paths, list) or not all(
        isinstance(path, str) for path in backend_paths
    ):
        raise RuntimeError("[build-system].backend-path must be a string array")
    sys.path[:0] = [str((project / path).resolve()) for path in backend_paths]
    module_name, separator, object_path = name.partition(":")
    backend: object = importlib.import_module(module_name)
    if separator:
        for component in object_path.split("."):
            backend = getattr(backend, component)
    hook = getattr(backend, "build_wheel", None)
    if not callable(hook):
        raise TypeError(f"PEP 517 backend {name!r} has no build_wheel hook")
    return cast(BuildWheel, hook)


def _validate_wheel(wheel: Path) -> None:
    """Verify every wheel payload identity inside the measured Bazel action."""
    try:
        archive = zipfile.ZipFile(wheel)
    except (OSError, zipfile.BadZipFile) as error:
        raise RuntimeError(f"invalid wheel {wheel.name!r}") from error
    with archive:
        files: dict[str, zipfile.ZipInfo] = {}
        for member in archive.infolist():
            path = Path(member.filename)
            if (
                path.is_absolute()
                or ".." in path.parts
                or "\\" in member.filename
                or member.filename in files
            ):
                raise RuntimeError(f"invalid wheel member {member.filename!r}")
            if not member.is_dir():
                files[member.filename] = member
        dist_infos = {
            Path(name).parts[0]
            for name in files
            if Path(name).parts[0].endswith(".dist-info")
        }
        if len(dist_infos) != 1:
            raise RuntimeError(
                f"wheel {wheel.name!r} contains {len(dist_infos)} dist-info directories"
            )
        dist_info = dist_infos.pop()
        record_path = f"{dist_info}/RECORD"
        for required in (f"{dist_info}/METADATA", f"{dist_info}/WHEEL", record_path):
            if required not in files:
                raise RuntimeError(f"wheel {wheel.name!r} has no {required}")
        try:
            wheel_metadata = Parser().parsestr(
                archive.read(f"{dist_info}/WHEEL").decode("utf-8")
            )
        except UnicodeDecodeError as error:
            raise RuntimeError(
                f"wheel {wheel.name!r} has invalid WHEEL metadata"
            ) from error
        components = wheel.name.removesuffix(".whl").split("-")
        if len(components) not in (5, 6):
            raise RuntimeError(f"wheel {wheel.name!r} has an invalid filename")
        python_tag, abi_tag, platform_tag = components[-3:]
        expected_tags = {
            f"{python}-{abi}-{platform}"
            for python in python_tag.split(".")
            for abi in abi_tag.split(".")
            for platform in platform_tag.split(".")
        }
        tags = wheel_metadata.get_all("Tag", [])
        if len(tags) != len(set(tags)) or set(tags) != expected_tags:
            raise RuntimeError(
                f"wheel {wheel.name!r} has conflicting compatibility tags"
            )
        try:
            rows = csv.reader(archive.read(record_path).decode("utf-8").splitlines())
        except UnicodeDecodeError as error:
            raise RuntimeError(f"wheel {wheel.name!r} has invalid RECORD") from error
        record: dict[str, tuple[str, str]] = {}
        for row in rows:
            if len(row) != 3 or row[0] in record:
                raise RuntimeError(
                    f"wheel {wheel.name!r} has invalid RECORD row {row!r}"
                )
            record[row[0]] = (row[1], row[2])
        signatures = {record_path + suffix for suffix in (".jws", ".p7s")}
        if (
            set(files) - signatures != set(record)
            or record.get(record_path) != ("", "")
            or set(record) & signatures
        ):
            raise RuntimeError(f"wheel {wheel.name!r} has incomplete RECORD coverage")
        for name, member in files.items():
            if name == record_path or name in signatures:
                continue
            encoded_hash, encoded_size = record[name]
            algorithm, separator, expected = encoded_hash.partition("=")
            if not separator or algorithm.lower() in {"md5", "sha1"}:
                raise RuntimeError(f"wheel RECORD has no strong hash for {name!r}")
            data = archive.read(member)
            try:
                actual = (
                    base64.urlsafe_b64encode(hashlib.new(algorithm, data).digest())
                    .rstrip(b"=")
                    .decode()
                )
            except ValueError as error:
                raise RuntimeError(
                    f"wheel RECORD uses unsupported hash {algorithm!r}"
                ) from error
            if actual != expected:
                raise RuntimeError(f"wheel RECORD hash mismatch for {name!r}")
            if not encoded_size.isdecimal() or int(encoded_size) != len(data):
                raise RuntimeError(f"wheel RECORD size mismatch for {name!r}")


def main() -> None:
    """Build one immutable wheel from only declared Bazel inputs."""
    args = _arguments()
    output = args.output.resolve()
    if not output.is_dir() or any(output.iterdir()):
        raise RuntimeError(f"Bazel did not provide an empty tree artifact at {output}")
    with tempfile.TemporaryDirectory(prefix="bsmr-bazel-pep517-") as temporary:
        project = Path(temporary) / "source"
        project.mkdir()
        _copy_project(args.source, project)
        os.chdir(project)
        filename = _backend(project)(str(output))
    wheels = sorted(output.glob("*.whl"))
    if len(wheels) != 1 or wheels[0].name != filename:
        raise RuntimeError(
            f"PEP 517 backend returned {filename!r} but produced {[path.name for path in wheels]!r}"
        )
    _validate_wheel(wheels[0])


if __name__ == "__main__":
    main()

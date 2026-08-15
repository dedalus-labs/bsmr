# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Executes native Python actions with exact tools and isolated mutable state.

"""Execute native Python actions with exact tools and isolated mutable state."""

from __future__ import annotations

import argparse
import base64
import configparser
import contextlib
import csv
import hashlib
import importlib
import json
import os
import platform
import pprint
import re
import runpy
import shutil
import site
import stat
import subprocess
import sys
import zipfile
from collections.abc import Iterator
from email.message import Message
from email.parser import Parser
from pathlib import Path, PurePosixPath
from typing import Final
from urllib.parse import quote

import tomllib

_SOURCE_DATE_EPOCH: Final = "315532800"
# Keep this baseline aligned with uv's Darwin target triples:
# https://github.com/astral-sh/uv/blob/0.12.4/crates/uv-configuration/src/target_triple.rs
_MACOSX_DEPLOYMENT_TARGET: Final = "13.0"
_MACOSX_TARGETS: Final = {
    "aarch64-apple-darwin": ("macosx-13.0-arm64", "arm64"),
    "x86_64-apple-darwin": ("macosx-13.0-x86_64", "x86_64"),
}
# Keep this finite catalog aligned with uv's python-build-standalone sysconfig
# mappings: https://github.com/astral-sh/uv/tree/0.12.4/crates/uv-python/src/sysconfig
_C_COMPILERS: Final = frozenset(
    {
        "clang",
        "musl-clang",
        "/usr/bin/aarch64-linux-gnu-gcc",
        "/usr/bin/arm-linux-gnueabi-gcc",
        "/usr/bin/arm-linux-gnueabihf-gcc",
        "/usr/bin/loongarch64-linux-gnu-gcc",
        "/usr/bin/mips-linux-gnu-gcc",
        "/usr/bin/mipsel-linux-gnu-gcc",
        "/usr/bin/powerpc64le-linux-gnu-gcc",
        "/usr/bin/riscv64-linux-gnu-clang",
        "/usr/bin/riscv64-linux-gnu-gcc",
        "/usr/bin/s390x-linux-gnu-gcc",
        "/usr/bin/x86_64-linux-gnu-gcc",
    }
)
_CXX_COMPILERS: Final = frozenset(
    {
        "clang++",
        "/usr/bin/aarch64-linux-gnu-g++",
        "/usr/bin/arm-linux-gnueabi-g++",
        "/usr/bin/arm-linux-gnueabihf-g++",
        "/usr/bin/loongarch64-linux-gnu-g++",
        "/usr/bin/mips-linux-gnu-g++",
        "/usr/bin/mipsel-linux-gnu-g++",
        "/usr/bin/powerpc64le-linux-gnu-g++",
        "/usr/bin/riscv64-linux-gnu-clang++",
        "/usr/bin/riscv64-linux-gnu-g++",
        "/usr/bin/s390x-linux-gnu-g++",
        "/usr/bin/x86_64-linux-gnu-g++",
    }
)
_COMPILER_VARIABLES: Final = {
    "BLDSHARED": (_C_COMPILERS, "cc"),
    "CC": (_C_COMPILERS, "cc"),
    "CXX": (_CXX_COMPILERS, "c++"),
    "LDCXXSHARED": (_CXX_COMPILERS, "c++"),
    "LDSHARED": (_C_COMPILERS, "cc"),
    "LINKCC": (_C_COMPILERS, "cc"),
}


def _arguments() -> argparse.Namespace:
    """Parse the runner's closed command-line contract."""
    parser = argparse.ArgumentParser(allow_abbrev=False)
    parser.add_argument(
        "mode",
        choices=(
            "install-wheel",
            "locked-package",
            "select-package",
            "compose-environment",
            "validate-environments",
            "ruff",
            "ty",
            "wheel",
            "wheel-environment",
        ),
    )
    parser.add_argument("--absent", action="store_true")
    parser.add_argument("--build-environment", action="append", default=[], type=Path)
    parser.add_argument("--artifact", action="append", default=[], type=Path)
    parser.add_argument("--config-setting", action="append", default=[])
    parser.add_argument("--distribution")
    parser.add_argument("--environment", action="append", default=[], type=Path)
    parser.add_argument("--lock", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--package-build-variable", action="append", default=[])
    parser.add_argument("--package-config-setting", action="append", default=[])
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--package", action="append", default=[], nargs=3)
    parser.add_argument("--project-root")
    parser.add_argument("--python", type=Path, required=True)
    parser.add_argument("--python-platform", required=True)
    parser.add_argument("--requirement")
    parser.add_argument("--ruff", type=Path)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--source-artifact", type=Path)
    parser.add_argument("--source-subdirectory")
    parser.add_argument("--source-tree", type=Path)
    parser.add_argument("--source-permitted", action="store_true")
    parser.add_argument("--ty", type=Path)
    parser.add_argument("--uv", type=Path)
    parser.add_argument("--vcs", type=Path)
    parser.add_argument("--version")
    parser.add_argument("--wheel-dir", action="append", default=[], type=Path)
    return parser.parse_args()


def _required[T](value: T | None, name: str) -> T:
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
        "CFLAGS": "-g0",
        "CXXFLAGS": "-g0",
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


def _configure_target_platform(
    python_platform: str, environment: dict[str, str], scratch: Path
) -> None:
    """Align backend outputs with uv's declared target-platform baseline."""
    target = _MACOSX_TARGETS.get(python_platform)
    if target is None:
        environment.pop("MACOSX_DEPLOYMENT_TARGET", None)
        environment.pop("_PYTHON_HOST_PLATFORM", None)
        return
    host_platform, machine = target
    environment["MACOSX_DEPLOYMENT_TARGET"] = _MACOSX_DEPLOYMENT_TARGET
    environment["_PYTHON_HOST_PLATFORM"] = host_platform
    shim = scratch / "target-platform"
    shim.mkdir(exist_ok=True)
    # packaging.tags reads the host kernel through platform.mac_ver(), even
    # when sysconfig and the compiler already target the declared platform.
    (shim / "sitecustomize.py").write_text(
        "import platform\n\n\n"
        "def _bsmr_mac_ver():\n"
        f"    return ('{_MACOSX_DEPLOYMENT_TARGET}.0', ('', '', ''), '{machine}')\n"
        "\n\nplatform.mac_ver = _bsmr_mac_ver\n",
        encoding="utf-8",
    )
    python_path = environment.get("PYTHONPATH")
    environment["PYTHONPATH"] = os.pathsep.join(
        path for path in (str(shim), python_path) if path
    )


def _patch_sysconfig_values(
    values: dict[str, object], python_root: Path
) -> dict[str, object]:
    """Relocate uv's pinned python-build-standalone compiler metadata."""
    patched = values.copy()
    for name, value in patched.items():
        if not isinstance(value, str):
            continue
        parts = iter(value.split())
        relocated: list[str] = []
        for part in parts:
            if part == "-isysroot":
                if next(parts, None) is None:
                    raise RuntimeError(f"{name} has an unterminated -isysroot")
                continue
            if part == "/install":
                part = str(python_root)
            elif part.startswith("/install/"):
                part = str(python_root / part.removeprefix("/install/"))
            relocated.append(part)
        patched[name] = " ".join(relocated)
    for name, (candidates, replacement) in _COMPILER_VARIABLES.items():
        value = patched.get(name)
        if value is None:
            continue
        if not isinstance(value, str):
            raise TypeError(f"unsupported {name} value {value!r}")
        words = value.split()
        words = [replacement if word in candidates else word for word in words]
        if not words or words[0] != replacement:
            raise RuntimeError(f"unsupported {name} value {value!r}")
        patched[name] = " ".join(words)
    if "AR" in patched:
        patched["AR"] = "ar"
    patched["PYTHON_BUILD_STANDALONE"] = 1
    return patched


def _configure_sysconfig(
    python: Path, scratch: Path, process_environment: dict[str, str]
) -> None:
    """Expose relocatable standalone build metadata to every child Python."""
    python_root = python.resolve().parent.parent
    candidates = list(python_root.glob("lib/python*/_sysconfigdata_*.py"))
    if len(candidates) != 1:
        raise RuntimeError(
            f"Python distribution contains {len(candidates)} sysconfig data files"
        )
    values = runpy.run_path(str(candidates[0])).get("build_time_vars")
    if not isinstance(values, dict) or not all(
        isinstance(name, str) for name in values
    ):
        raise RuntimeError("Python distribution has invalid sysconfig data")
    patched = _patch_sysconfig_values(values, python_root)
    directory = scratch / "sysconfig"
    directory.mkdir()
    (directory / "_bsmr_sysconfigdata.py").write_text(
        "build_time_vars = " + pprint.pformat(patched, sort_dicts=True) + "\n",
        encoding="utf-8",
    )
    process_environment["_PYTHON_SYSCONFIGDATA_NAME"] = "_bsmr_sysconfigdata"
    process_environment["_PYTHON_SYSCONFIGDATA_PATH"] = str(directory)


def _run(
    command: list[str], environment: dict[str, str], cwd: Path | None = None
) -> None:
    """Run one exact command and propagate its nonzero status."""
    completed = subprocess.run(command, check=False, cwd=cwd, env=environment)
    if completed.returncode != 0:
        raise SystemExit(completed.returncode)


def _uv_config_arguments(args: argparse.Namespace, scratch: Path) -> list[str]:
    """Write typed package build variables to one isolated uv configuration."""
    if not args.package_build_variable:
        return []
    packages: dict[str, dict[str, str]] = {}
    for setting in args.package_build_variable:
        package, separator, assignment = setting.partition(":")
        name, equals, value = assignment.partition("=")
        if not separator or not package or not equals or not name:
            raise ValueError(f"invalid package build variable {setting!r}")
        variables = packages.setdefault(package, {})
        if name in variables:
            raise ValueError(f"duplicate package build variable {package}:{name}")
        variables[name] = value
    config = scratch / "uv.toml"
    lines = []
    for package, variables in sorted(packages.items()):
        lines.append(f"[extra-build-variables.{json.dumps(package)}]")
        lines.extend(
            f"{json.dumps(name)} = {json.dumps(value)}"
            for name, value in sorted(variables.items())
        )
    config.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return ["--config-file", str(config)]


def _config_settings(
    global_entries: list[str], package_entries: list[str], distribution: str
) -> dict[str, str | list[str]]:
    """Reproduce uv's package-first PEP 517 config-setting merge contract."""
    merged: dict[str, list[str]] = {}

    def add(entry: str) -> None:
        """Append one validated hook setting while preserving repeated values."""
        name, separator, value = entry.partition("=")
        name = name.strip()
        value = value.strip()
        if not separator or not name:
            raise ValueError(f"invalid PEP 517 config setting {entry!r}")
        merged.setdefault(name, []).append(value)

    for entry in package_entries:
        package, separator, setting = entry.partition(":")
        if (
            not separator
            or _normalize_distribution_name(package.strip()) != distribution
        ):
            raise ValueError(
                f"package config setting {entry!r} does not select {distribution!r}"
            )
        add(setting)
    for entry in global_entries:
        add(entry)
    return {
        name: values[0] if len(values) == 1 else values
        for name, values in sorted(merged.items())
    }


def _package_build_variables(
    entries: list[str], distribution: str, environment: dict[str, str]
) -> None:
    """Apply only the selected project's declared backend environment variables."""
    variables: set[str] = set()
    for entry in entries:
        package, separator, assignment = entry.partition(":")
        name, equals, value = assignment.partition("=")
        if (
            not separator
            or _normalize_distribution_name(package.strip()) != distribution
            or not equals
            or not name
            or "=" in name
        ):
            raise ValueError(f"invalid package build variable {entry!r}")
        if name in variables:
            raise ValueError(f"duplicate package build variable {distribution}:{name}")
        variables.add(name)
        environment[name] = value


def _backend_specification(project: Path) -> tuple[str, list[Path]]:
    """Load one explicit PEP 517 backend and its contained import roots."""
    manifest = project / "pyproject.toml"
    try:
        document = tomllib.loads(manifest.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise RuntimeError(
            f"project has no valid pyproject.toml at '{manifest}'"
        ) from error
    build_system = document.get("build-system")
    if not isinstance(build_system, dict):
        raise TypeError("project pyproject.toml has no [build-system] table")
    backend = build_system.get("build-backend")
    if not isinstance(backend, str) or not backend.strip():
        raise RuntimeError("project [build-system] has no explicit build-backend")
    raw_paths = build_system.get("backend-path", [])
    if not isinstance(raw_paths, list) or not all(
        isinstance(path, str) for path in raw_paths
    ):
        raise RuntimeError("project [build-system].backend-path must be a string array")
    paths: list[Path] = []
    for value in raw_paths:
        if value == ".":
            path = project
        else:
            try:
                relative = _normalized_source_subdirectory(value)
            except ValueError as error:
                raise RuntimeError(
                    f"project build-system backend-path {value!r} is not contained"
                ) from error
            path = project.joinpath(*relative.parts).resolve()
        if (path != project and project not in path.parents) or not path.is_dir():
            raise RuntimeError(
                f"project build-system backend-path {value!r} is not a contained directory"
            )
        paths.append(path)
    return backend.strip(), paths


def _load_backend(reference: str) -> object:
    """Import one PEP 517 backend object from its declared module reference."""
    module_name, separator, object_path = reference.partition(":")
    if not module_name or (separator and not object_path):
        raise RuntimeError(f"invalid PEP 517 build-backend {reference!r}")
    backend: object = importlib.import_module(module_name)
    for component in object_path.split(".") if separator else []:
        if not component.isidentifier():
            raise RuntimeError(f"invalid PEP 517 build-backend {reference!r}")
        try:
            backend = getattr(backend, component)
        except AttributeError as error:
            raise RuntimeError(
                f"PEP 517 build-backend {reference!r} has no object {object_path!r}"
            ) from error
    return backend


@contextlib.contextmanager
def _backend_process(
    python: Path,
    backend_paths: list[Path],
    environments: list[Path],
    environment: dict[str, str],
    python_platform: str,
) -> Iterator[None]:
    """Expose only the pinned interpreter, build closure, and action environment."""
    if Path(sys.executable).resolve() != python.resolve():
        raise RuntimeError(
            f"Python runner interpreter '{sys.executable}' does not match '{python}'"
        )
    previous_environment = os.environ.copy()
    previous_path = sys.path.copy()
    previous_mac_ver = platform.mac_ver
    python_root = python.resolve().parent.parent
    standard_library = []
    for value in previous_path:
        if not value:
            continue
        path = Path(value).resolve()
        if (
            (path == python_root or python_root in path.parents)
            and "site-packages" not in path.parts
            and "dist-packages" not in path.parts
        ):
            standard_library.append(str(path))
    if not standard_library:
        raise RuntimeError(
            f"pinned Python '{python}' exposes no standard-library paths"
        )
    os.environ.clear()
    os.environ.update(environment)
    sys.path[:] = [*(str(path) for path in backend_paths), *standard_library]
    for root in environments:
        site.addsitedir(str(root))
    for value in sys.path:
        path = Path(value).resolve()
        declared = any(
            path == root or root in path.parents
            for root in [*backend_paths, *environments]
        )
        standard = (
            (path == python_root or python_root in path.parents)
            and "site-packages" not in path.parts
            and "dist-packages" not in path.parts
        )
        if not declared and not standard:
            raise RuntimeError(
                f"build environment added undeclared import path '{path}'"
            )
    target = _MACOSX_TARGETS.get(python_platform)
    if target is not None:
        _, machine = target
        platform.mac_ver = lambda: (
            f"{_MACOSX_DEPLOYMENT_TARGET}.0",
            ("", "", ""),
            machine,
        )
    try:
        yield
    finally:
        platform.mac_ver = previous_mac_ver
        sys.path[:] = previous_path
        os.environ.clear()
        os.environ.update(previous_environment)


def _record_digest(data: bytes) -> str:
    """Return a wheel RECORD-compatible SHA-256 digest."""
    digest = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=")
    return f"sha256={digest.decode('ascii')}"


def _wheel_path(value: str) -> PurePosixPath:
    """Accept one canonical wheel member path contained by its archive."""
    path = PurePosixPath(value)
    if (
        not path.parts
        or path.is_absolute()
        or ".." in path.parts
        or "\\" in value
        or (len(value) > 1 and value[1] == ":")
        or path.as_posix() != value
    ):
        raise RuntimeError(f"wheel member path {value!r} is not normalized")
    return path


def _wheel_files(archive: zipfile.ZipFile) -> dict[str, zipfile.ZipInfo]:
    """Index regular wheel members while rejecting ambiguous filesystem kinds."""
    files: dict[str, zipfile.ZipInfo] = {}
    for member in archive.infolist():
        name = member.filename.removesuffix("/") if member.is_dir() else member.filename
        _wheel_path(name)
        mode = member.external_attr >> 16
        kind = stat.S_IFMT(mode)
        if kind not in (0, stat.S_IFREG, stat.S_IFDIR):
            raise RuntimeError(f"wheel member {name!r} has unsupported filesystem kind")
        if member.is_dir():
            continue
        if name in files:
            raise RuntimeError(f"wheel contains duplicate member {name!r}")
        files[name] = member
    return files


def _wheel_record(
    archive: zipfile.ZipFile,
    files: dict[str, zipfile.ZipInfo],
    record_path: str,
) -> dict[str, tuple[str, str]]:
    """Load a complete RECORD whose payload identities can be verified."""
    try:
        rows = csv.reader(archive.read(record_path).decode("utf-8").splitlines())
    except (KeyError, UnicodeDecodeError) as error:
        raise RuntimeError(f"wheel has no valid {record_path}") from error
    record: dict[str, tuple[str, str]] = {}
    for row in rows:
        if len(row) != 3:
            raise RuntimeError(f"wheel RECORD row must have three fields: {row!r}")
        path = _wheel_path(row[0]).as_posix()
        if path in record:
            raise RuntimeError(f"wheel RECORD contains duplicate path {path!r}")
        record[path] = (row[1], row[2])
    signatures = {record_path + suffix for suffix in (".jws", ".p7s")}
    if record.get(record_path) != ("", "") or set(record) & signatures:
        raise RuntimeError("wheel RECORD has invalid self or signature entries")
    missing = sorted(set(files) - set(record) - signatures)
    extra = sorted(set(record) - set(files))
    if missing or extra:
        raise RuntimeError(
            f"wheel RECORD does not match archive; missing={missing!r}, extra={extra!r}"
        )
    return record


def _verify_record_entry(
    path: str,
    data: bytes,
    record: dict[str, tuple[str, str]],
    record_path: str,
) -> None:
    """Verify one wheel member against its mandatory strong RECORD identity."""
    if path == record_path or path in {record_path + ".jws", record_path + ".p7s"}:
        return
    encoded_hash, encoded_size = record[path]
    algorithm, separator, expected = encoded_hash.partition("=")
    if not separator or algorithm.lower() in {"md5", "sha1"}:
        raise RuntimeError(f"wheel RECORD has no strong hash for {path!r}")
    try:
        digest = hashlib.new(algorithm, data).digest()
    except ValueError as error:
        raise RuntimeError(
            f"wheel RECORD uses unsupported hash {algorithm!r} for {path!r}"
        ) from error
    actual = base64.urlsafe_b64encode(digest).rstrip(b"=").decode("ascii")
    if actual != expected:
        raise RuntimeError(f"wheel RECORD hash mismatch for {path!r}")
    if not encoded_size.isdecimal() or int(encoded_size) != len(data):
        raise RuntimeError(f"wheel RECORD size mismatch for {path!r}")


def _wheel_release_identity(
    wheel: Path, dist_info: str, name: str | None, version: str | None
) -> tuple[str, str]:
    """Require the filename, metadata directory, and core metadata to agree."""
    components = wheel.name.removesuffix(".whl").split("-")
    if (
        not wheel.name.endswith(".whl")
        or len(components) not in (5, 6)
        or any(not component for component in components)
        or (len(components) == 6 and not components[2][0].isdigit())
    ):
        raise RuntimeError(f"wheel has invalid filename {wheel.name!r}")
    directory_name, separator, directory_version = dist_info.removesuffix(
        ".dist-info"
    ).rpartition("-")
    if (
        not separator
        or not directory_name
        or not directory_version
        or not name
        or not version
    ):
        raise RuntimeError(f"wheel {wheel.name!r} has incomplete core metadata")
    distribution = _normalize_distribution_name(name)
    if (
        _normalize_distribution_name(components[0]) != distribution
        or _normalize_distribution_name(directory_name) != distribution
        or components[1] != directory_version
        or components[1] != version
    ):
        raise RuntimeError(f"wheel {wheel.name!r} has conflicting release identity")
    return distribution, version


def _validate_wheel_metadata(wheel: Path, metadata: Message) -> None:
    """Require WHEEL tags and build identity to match the archive filename."""
    components = wheel.name.removesuffix(".whl").split("-")
    python_tag, abi_tag, platform_tag = components[-3:]
    expected_tags = {
        f"{python}-{abi}-{platform}"
        for python in python_tag.split(".")
        for abi in abi_tag.split(".")
        for platform in platform_tag.split(".")
    }
    tags = metadata.get_all("Tag", [])
    if len(tags) != len(set(tags)) or set(tags) != expected_tags:
        raise RuntimeError(f"wheel {wheel.name!r} has conflicting compatibility tags")
    expected_build = components[2] if len(components) == 6 else None
    if metadata.get("Build") != expected_build:
        raise RuntimeError(f"wheel {wheel.name!r} has conflicting build identity")


def _validate_built_wheel(wheel: Path, distribution: str) -> None:
    """Verify one backend result before admitting it to the action cache."""
    try:
        archive = zipfile.ZipFile(wheel)
    except (OSError, zipfile.BadZipFile) as error:
        raise RuntimeError(
            f"PEP 517 backend returned invalid wheel {wheel.name!r}"
        ) from error
    with archive:
        files = _wheel_files(archive)
        dist_infos = {
            _wheel_path(path).parts[0]
            for path in files
            if _wheel_path(path).parts[0].endswith(".dist-info")
        }
        if len(dist_infos) != 1:
            raise RuntimeError(
                f"wheel {wheel.name!r} contains {len(dist_infos)} dist-info directories"
            )
        dist_info = dist_infos.pop()
        record_path = f"{dist_info}/RECORD"
        record = _wheel_record(archive, files, record_path)
        try:
            metadata = Parser().parsestr(
                archive.read(f"{dist_info}/METADATA").decode("utf-8")
            )
            wheel_metadata = Parser().parsestr(
                archive.read(f"{dist_info}/WHEEL").decode("utf-8")
            )
        except (KeyError, UnicodeDecodeError) as error:
            raise RuntimeError(
                f"wheel {wheel.name!r} has incomplete metadata"
            ) from error
        name = metadata.get("Name")
        version = metadata.get("Version")
        wheel_distribution, _ = _wheel_release_identity(wheel, dist_info, name, version)
        if wheel_distribution != distribution:
            raise RuntimeError(
                f"wheel {wheel.name!r} metadata does not identify {distribution!r}"
            )
        if not wheel_metadata.get("Wheel-Version", "").startswith(
            "1."
        ) or wheel_metadata.get("Root-Is-Purelib", "") not in {"true", "false"}:
            raise RuntimeError(f"wheel {wheel.name!r} uses unsupported wheel metadata")
        _validate_wheel_metadata(wheel, wheel_metadata)
        for path, member in files.items():
            _verify_record_entry(path, archive.read(member), record, record_path)


def _build_wheel(
    backend: object,
    output: Path,
    config_settings: dict[str, str | list[str]],
    distribution: str,
) -> None:
    """Execute the PEP 517 wheel hooks with a fully precomputed build closure."""
    build = getattr(backend, "build_wheel", None)
    if not callable(build):
        raise TypeError("PEP 517 backend has no callable build_wheel hook")
    output.mkdir()
    filename = build(str(output), config_settings, None)
    if (
        not isinstance(filename, str)
        or PurePosixPath(filename).parts != (filename,)
        or not filename.endswith(".whl")
    ):
        raise RuntimeError(
            f"PEP 517 build_wheel returned invalid filename {filename!r}"
        )
    wheel = output / filename
    artifacts = list(output.iterdir())
    if artifacts != [wheel] or not wheel.is_file():
        raise RuntimeError(
            f"PEP 517 backend produced unexpected artifacts {[path.name for path in artifacts]!r}"
        )
    _validate_built_wheel(wheel, distribution)


def _entry_points(archive: zipfile.ZipFile, path: str) -> dict[str, str]:
    """Parse console and GUI entry points using the standard case-sensitive INI form."""
    try:
        source = archive.read(path).decode("utf-8")
    except KeyError:
        return {}
    except UnicodeDecodeError as error:
        raise RuntimeError(f"wheel has invalid UTF-8 entry points at {path}") from error
    parser = configparser.ConfigParser(
        interpolation=None,
        delimiters=("=",),
        comment_prefixes=("#", ";"),
        strict=True,
    )
    parser.optionxform = str
    try:
        parser.read_string(source)
    except configparser.Error as error:
        raise RuntimeError(f"wheel has invalid entry points at {path}") from error
    if parser.defaults():
        raise RuntimeError(f"wheel entry points at {path} contain inherited defaults")
    scripts: dict[str, str] = {}
    pattern = re.compile(
        r"^(?P<module>[\w.-]+)\s*:\s*(?P<function>[\w.-]+)"
        r"(?:\s*\[[^]]*\])?\s*$"
    )
    for section in ("console_scripts", "gui_scripts"):
        for name, value in parser.items(section) if parser.has_section(section) else []:
            if name in scripts or _wheel_path(f"bin/{name}").parts != ("bin", name):
                raise RuntimeError(f"wheel has invalid entry-point name {name!r}")
            match = pattern.fullmatch(value)
            if match is None:
                raise RuntimeError(f"wheel has invalid entry point {name!r}={value!r}")
            module = match.group("module")
            function = match.group("function")
            if not all(part.isidentifier() for part in module.split(".")) or not all(
                part.isidentifier() for part in function.split(".")
            ):
                raise RuntimeError(f"wheel has invalid entry point {name!r}={value!r}")
            imported = function.split(".", 1)[0]
            scripts[name] = (
                "#!/usr/bin/env python3\n"
                "# -*- coding: utf-8 -*-\n"
                "import sys\n"
                f"from {module} import {imported}\n"
                'if __name__ == "__main__":\n'
                '    if sys.argv[0].endswith("-script.pyw"):\n'
                "        sys.argv[0] = sys.argv[0][:-11]\n"
                '    elif sys.argv[0].endswith(".exe"):\n'
                "        sys.argv[0] = sys.argv[0][:-4]\n"
                f"    sys.exit({function}())\n"
            )
    return scripts


def _wheel_destination(
    path: PurePosixPath,
    data_prefix: str,
    distribution: str,
    output: Path,
) -> tuple[Path, bool]:
    """Map one wheel archive path through the wheel installation scheme."""
    if path.parts[0] != data_prefix:
        if path.parts[0].endswith(".data"):
            raise RuntimeError(
                f"wheel data member {path.as_posix()!r} does not match {data_prefix!r}"
            )
        return output.joinpath(*path.parts), False
    if len(path.parts) < 3:
        raise RuntimeError(f"wheel data member {path.as_posix()!r} has no payload path")
    scheme, relative = path.parts[1], path.parts[2:]
    if scheme in {"purelib", "platlib", "data"}:
        root = output
    elif scheme == "scripts":
        root = output / "bin"
    elif scheme == "headers":
        root = output / "include" / distribution
    else:
        raise RuntimeError(f"wheel has unknown data scheme {scheme!r}")
    return root.joinpath(*relative), scheme == "scripts"


def _write_installed_record(
    output: Path,
    record: Path,
    files: dict[Path, tuple[str, int]],
) -> None:
    """Describe the exact installed tree after data spreading and script generation."""
    rows = [
        [path.relative_to(output).as_posix(), digest, str(size)]
        for path, (digest, size) in files.items()
    ]
    rows.append([record.relative_to(output).as_posix(), "", ""])
    with record.open("w", encoding="utf-8", newline="") as destination:
        csv.writer(destination, lineterminator="\n").writerows(sorted(rows))


def _install_wheel(wheel: Path, output: Path) -> None:
    """Install one already-selected wheel without invoking a package resolver."""
    with zipfile.ZipFile(wheel) as archive:
        files = _wheel_files(archive)
        dist_infos = {
            _wheel_path(path).parts[0]
            for path in files
            if _wheel_path(path).parts[0].endswith(".dist-info")
        }
        if len(dist_infos) != 1:
            raise RuntimeError(
                f"wheel {wheel.name!r} contains {len(dist_infos)} dist-info directories"
            )
        dist_info = dist_infos.pop()
        prefix = dist_info.removesuffix(".dist-info")
        record_path = f"{dist_info}/RECORD"
        record = _wheel_record(archive, files, record_path)
        try:
            metadata = Parser().parsestr(
                archive.read(f"{dist_info}/METADATA").decode("utf-8")
            )
            wheel_metadata = Parser().parsestr(
                archive.read(f"{dist_info}/WHEEL").decode("utf-8")
            )
        except (KeyError, UnicodeDecodeError) as error:
            raise RuntimeError(
                f"wheel {wheel.name!r} has incomplete metadata"
            ) from error
        name = metadata.get("Name")
        version = metadata.get("Version")
        distribution, _ = _wheel_release_identity(wheel, dist_info, name, version)
        wheel_version = wheel_metadata.get("Wheel-Version", "")
        root_is_purelib = wheel_metadata.get("Root-Is-Purelib", "")
        if not wheel_version.startswith("1.") or root_is_purelib not in {
            "true",
            "false",
        }:
            raise RuntimeError(f"wheel {wheel.name!r} uses unsupported wheel metadata")
        _validate_wheel_metadata(wheel, wheel_metadata)
        scripts = _entry_points(archive, f"{dist_info}/entry_points.txt")
        data_prefix = f"{prefix}.data"
        installed: dict[Path, tuple[str, int]] = {}
        for path, member in files.items():
            if path == record_path:
                continue
            source = _wheel_path(path)
            destination, data_script = _wheel_destination(
                source, data_prefix, distribution, output
            )
            data = archive.read(member)
            _verify_record_entry(path, data, record, record_path)
            if data_script and destination.name in scripts:
                continue
            if data_script and data.startswith(b"#!python"):
                _, separator, body = data.partition(b"\n")
                if not separator:
                    raise RuntimeError(
                        f"wheel script {path!r} has no shebang terminator"
                    )
                data = b"#!/usr/bin/env python3\n" + body
            if destination.exists():
                raise RuntimeError(
                    f"wheels contain duplicate installed path {destination!r}"
                )
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
            mode = member.external_attr >> 16 & 0o777
            destination.chmod((mode or 0o644) | (0o111 if data_script else 0))
            installed[destination] = (_record_digest(data), len(data))
        for name, script in scripts.items():
            destination = output / "bin" / name
            data = script.encode()
            if destination.exists():
                if destination.read_bytes() != data:
                    raise RuntimeError(
                        f"wheel entry point conflicts at {destination!r}"
                    )
            else:
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(data)
                destination.chmod(0o755)
            installed[destination] = (_record_digest(data), len(data))
        installed_record = output / record_path
        installed_record.parent.mkdir(parents=True, exist_ok=True)
        _write_installed_record(output, installed_record, installed)


def _install_wheels(wheels: list[Path], output: Path) -> None:
    """Install an exact wheel set into one deterministic package root."""
    output.mkdir()
    for wheel in wheels:
        _install_wheel(wheel.resolve(), output)


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
            if body.startswith(b"'''exec' "):
                _, separator, body = body.partition(b"\n' '''\n")
                if not separator:
                    raise RuntimeError(
                        f"entry point '{script}' has an unterminated uv trampoline"
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


def _normalized_source_subdirectory(subdirectory: str) -> PurePosixPath:
    """Parse one portable project root contained by an acquired source."""
    path = PurePosixPath(subdirectory)
    if (
        not path.parts
        or path.is_absolute()
        or ".." in path.parts
        or "\\" in subdirectory
        or (len(subdirectory) > 1 and subdirectory[1] == ":")
        or path.as_posix() != subdirectory
    ):
        raise ValueError(
            f"source subdirectory '{subdirectory}' is not a normalized relative path"
        )
    return path


def _source_artifact_reference(artifact: Path, subdirectory: str | None) -> str:
    """Address one acquired archive and an optional normalized project root."""
    artifact = artifact.resolve()
    if subdirectory is None:
        return str(artifact)
    _normalized_source_subdirectory(subdirectory)
    return f"{artifact.as_uri()}#subdirectory={quote(subdirectory, safe='/')}"


def _source_tree_reference(tree: Path, subdirectory: str | None) -> str:
    """Select one normalized project root contained by an acquired source tree."""
    root = tree.resolve()
    if not root.is_dir():
        raise RuntimeError(f"acquired source tree '{tree}' is not a directory")
    source = (
        root
        if subdirectory is None
        else root.joinpath(
            *_normalized_source_subdirectory(subdirectory).parts
        ).resolve()
    )
    if (source != root and root not in source.parents) or not source.is_dir():
        raise RuntimeError(
            f"source project root '{source}' is not a contained directory"
        )
    return str(source)


def _normalize_distribution_name(name: str) -> str:
    """Normalize one core-metadata Name using the Python packaging contract."""
    normalized: list[str] = []
    separator = False
    for character in name.lower():
        if character.isascii() and character.isalnum():
            normalized.append(character)
            separator = False
        elif character in "-_.":
            if not separator:
                normalized.append("-")
                separator = True
        else:
            raise RuntimeError(f"distribution metadata has invalid Name {name!r}")
    value = "".join(normalized)
    if not value or value.startswith("-") or value.endswith("-"):
        raise RuntimeError(f"distribution metadata has invalid Name {name!r}")
    return value


def _validate_distribution_identity(
    packages: Path, distribution: str, version: str
) -> None:
    """Require one built wheel to match its locked project identity."""
    metadata_files = sorted(packages.glob("*.dist-info/METADATA"))
    if len(metadata_files) != 1:
        raise RuntimeError(
            f"expected {distribution}=={version}, installed {len(metadata_files)} metadata records"
        )
    metadata = Parser().parsestr(metadata_files[0].read_text(encoding="utf-8"))
    actual_name = metadata.get("Name")
    actual_version = metadata.get("Version")
    if not actual_name or not actual_version:
        raise RuntimeError(
            f"expected {distribution}=={version}, installed incomplete metadata"
        )
    if (
        _normalize_distribution_name(actual_name) != distribution
        or actual_version != version
    ):
        raise RuntimeError(
            f"expected {distribution}=={version}, installed {actual_name}=={actual_version}"
        )


def _activate_environment(root: Path, process_environment: dict[str, str]) -> Path:
    """Expose one exact CAS-backed package tree to a Python process."""
    environment = root.resolve()
    if not environment.is_dir():
        raise RuntimeError(f"Python environment '{root}' is not a directory")
    process_environment["PATH"] = os.pathsep.join(
        (str(environment / "bin"), process_environment["PATH"])
    )
    python_path = process_environment.get("PYTHONPATH")
    process_environment["PYTHONPATH"] = os.pathsep.join(
        path for path in (str(environment), python_path) if path
    )
    return environment


def _project_environments(
    args: argparse.Namespace, process_environment: dict[str, str]
) -> list[Path]:
    """Activate every declared project environment in argument order."""
    if not args.environment:
        raise ValueError("--environment is required for this action")
    return [
        _activate_environment(environment, process_environment)
        for environment in args.environment
    ]


def _locked_package(
    args: argparse.Namespace, process_environment: dict[str, str], scratch: Path
) -> None:
    """Install one normalized PEP 751 package against its build closure."""
    packages = args.output.resolve()
    if args.requirement is not None and not args.artifact:
        raise ValueError("an exact requirement requires locked wheel artifacts")
    source_inputs = sum(
        source is not None for source in (args.source_artifact, args.source_tree)
    )
    if source_inputs > 1:
        raise ValueError("source artifacts and source trees are mutually exclusive")
    source_input = source_inputs == 1
    if source_input and (args.artifact or args.requirement is not None):
        raise ValueError("wheel artifacts and source inputs are mutually exclusive")
    if source_input and not args.build_environment:
        raise ValueError("a source input requires a declared build environment")
    if source_input and (args.distribution is None or args.version is None):
        raise ValueError("a source input requires its locked identity")
    if not source_input and (
        args.source_subdirectory is not None or args.version is not None
    ):
        raise ValueError("source metadata requires a source input")
    if args.absent:
        if (
            args.artifact
            or args.source_artifact is not None
            or args.source_tree is not None
            or args.build_environment
            or args.config_setting
            or args.package_config_setting
            or args.package_build_variable
            or args.requirement is not None
        ):
            raise ValueError("an absent locked package cannot have installation inputs")
        packages.mkdir()
    elif args.artifact:
        if (
            args.build_environment
            or args.config_setting
            or args.package_config_setting
            or args.package_build_variable
        ):
            raise ValueError("a locked wheel artifact cannot have source-build inputs")
        if args.requirement is None and len(args.artifact) != 1:
            raise ValueError(
                "multiple locked wheel artifacts require an exact requirement"
            )
        if args.requirement is None:
            _install_wheels(args.artifact, packages)
        else:
            uv = _required(args.uv, "--uv")
            packages.mkdir()
            candidate_arguments = [
                argument
                for artifact in args.artifact
                for argument in ("--find-links", str(artifact.resolve().parent))
            ]
            command = [
                str(uv),
                "pip",
                "install",
                args.requirement,
                *candidate_arguments,
                "--target",
                str(packages),
                "--python",
                str(args.python),
                "--python-platform",
                args.python_platform,
                "--no-python-downloads",
                "--no-build",
                "--no-deps",
                "--no-index",
                "--offline",
                "--strict",
                "--color",
                "never",
                "--no-progress",
            ]
            _run(command, process_environment)
    else:
        uv = _required(args.uv, "--uv")
        packages.mkdir()
        build_flags = ["--no-build"]
        if args.build_environment:
            for environment in args.build_environment:
                _activate_environment(environment, process_environment)
            _configure_target_platform(
                args.python_platform, process_environment, scratch
            )
            build_flags = ["--no-build-isolation"]
        if source_input:
            source_reference = (
                _source_artifact_reference(
                    args.source_artifact, args.source_subdirectory
                )
                if args.source_artifact is not None
                else _source_tree_reference(args.source_tree, args.source_subdirectory)
            )
            command = [
                str(uv),
                *_uv_config_arguments(args, scratch),
                "pip",
                "install",
                source_reference,
                "--target",
                str(packages),
                "--python",
                str(args.python),
                "--python-platform",
                args.python_platform,
                "--no-python-downloads",
                *build_flags,
                "--no-deps",
                "--no-index",
                "--offline",
                "--strict",
                "--color",
                "never",
                "--no-progress",
            ]
        else:
            command = [
                str(uv),
                *_uv_config_arguments(args, scratch),
                "pip",
                "sync",
                str(_required(args.lock, "--lock")),
                "--target",
                str(packages),
                "--python",
                str(args.python),
                "--python-platform",
                args.python_platform,
                "--no-python-downloads",
                *build_flags,
                "--strict",
                "--preview-features",
                "pylock",
                "--color",
                "never",
                "--no-progress",
            ]
        command.extend(f"--config-setting={setting}" for setting in args.config_setting)
        command.extend(
            f"--config-settings-package={setting}"
            for setting in args.package_config_setting
        )
        _run(
            command,
            process_environment,
        )
        if source_input:
            _validate_distribution_identity(
                packages,
                str(_required(args.distribution, "--distribution")),
                str(_required(args.version, "--version")),
            )
    _normalize_entry_points(packages)
    _write_package_manifest(packages, Path(_required(args.manifest, "--manifest")))


def _select_locked_package(
    args: argparse.Namespace, process_environment: dict[str, str], scratch: Path
) -> None:
    """Ask pinned uv whether the lock selects a compatible wheel."""
    distribution = str(_required(args.distribution, "--distribution"))
    command = [
        str(_required(args.uv, "--uv")),
        "pip",
        "sync",
        str(_required(args.lock, "--lock")),
        "--target",
        str(scratch / "target"),
        "--python",
        str(args.python),
        "--python-platform",
        args.python_platform,
        "--dry-run",
        "--offline",
        "--only-binary",
        ":all:",
        "--no-python-downloads",
        "--strict",
        "--preview-features",
        "pylock",
        "--color",
        "never",
        "--no-progress",
    ]
    completed = subprocess.run(
        command,
        check=False,
        capture_output=True,
        env=process_environment,
        text=True,
    )
    if completed.returncode == 0:
        requirements = [
            line.removeprefix(" + ")
            for line in completed.stderr.splitlines()
            if line.startswith(" + ")
        ]
        if not requirements:
            args.output.write_text('{"acquisition":"absent"}\n', encoding="utf-8")
            return
        if len(requirements) != 1:
            raise RuntimeError(
                f"uv selected {len(requirements)} packages for locked distribution "
                f"'{distribution}'"
            )
        requirement = requirements[0]
        name, separator, version = requirement.partition("==")
        if (
            name != distribution
            or not separator
            or not version
            or any(character.isspace() for character in version)
        ):
            raise RuntimeError(
                f"uv emitted invalid locked requirement {requirement!r} for "
                f"'{distribution}'"
            )
        args.output.write_text(
            json.dumps(
                {"acquisition": "wheel", "requirement": requirement},
                separators=(",", ":"),
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )
        return
    no_wheel = (
        f"error: Package `{distribution}` can't be installed because it is marked "
        "as `--no-build` but has no binary distribution\n"
    )
    if (
        args.source_permitted
        and completed.returncode == 2
        and completed.stderr.endswith(no_wheel)
    ):
        args.output.write_text('{"acquisition":"source"}\n', encoding="utf-8")
        return
    sys.stdout.write(completed.stdout)
    sys.stderr.write(completed.stderr)
    raise SystemExit(completed.returncode)


def _file_digest(path: Path) -> str:
    """Hash one installed file without loading an extension binary into memory."""
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def _manifest_relative_path(value: object) -> str:
    """Accept only canonical portable paths contained by an environment root."""
    if not isinstance(value, str):
        raise TypeError("Python package manifest path must be a string")
    path = PurePosixPath(value)
    if (
        not path.parts
        or path.is_absolute()
        or ".." in path.parts
        or "\\" in value
        or (len(value) > 1 and value[1] == ":")
        or path.as_posix() != value
    ):
        raise RuntimeError(f"Python package manifest path '{value}' is not normalized")
    return value


def _write_package_manifest(package: Path, manifest: Path) -> None:
    """Record every installed path's kind, mode, and immutable content identity."""
    _validate_environment(package)
    entries = []
    for path in sorted(package.rglob("*")):
        relative = path.relative_to(package).as_posix()
        if path.is_dir():
            entries.append([relative, "directory"])
        else:
            entries.append(
                [
                    relative,
                    "file",
                    stat.S_IMODE(path.stat().st_mode),
                    _file_digest(path),
                ]
            )
    manifest.write_text(
        json.dumps(entries, separators=(",", ":")) + "\n", encoding="utf-8"
    )


def _environment_packages(
    args: argparse.Namespace,
) -> list[tuple[str, Path, list[list[object]]]]:
    """Load package manifests in canonical distribution-name order."""
    packages = []
    for name, manifest, root in args.package:
        entries = json.loads(Path(manifest).read_text(encoding="utf-8"))
        packages.append((str(name), Path(root), entries))
    packages.sort(key=lambda package: package[0])
    names = [name for name, _, _ in packages]
    if len(names) != len(set(names)):
        raise RuntimeError("Python environment contains duplicate package identities")
    return packages


def _metadata_import_name(value: str, field: str) -> str | None:
    """Parse one PEP 794 import declaration into its ownership identity."""
    name, separator, qualifier = value.partition(";")
    name = name.strip()
    if separator and qualifier.strip() != "private":
        raise RuntimeError(f"Python metadata has invalid {field} value {value!r}")
    if not name:
        if field == "Import-Name" and not separator:
            return None
        raise RuntimeError(f"Python metadata has invalid {field} value {value!r}")
    if not all(component.isidentifier() for component in name.split(".")):
        raise RuntimeError(f"Python metadata has invalid {field} value {value!r}")
    return name


def _is_distribution_metadata(entry: list[object]) -> bool:
    """Return whether one manifest entry is top-level wheel core metadata."""
    path = PurePosixPath(_manifest_relative_path(entry[0]))
    return (
        len(path.parts) == 2
        and path.parts[0].endswith(".dist-info")
        and path.parts[1] == "METADATA"
    )


def _package_imports(
    package: tuple[str, Path, list[list[object]]],
) -> tuple[str, set[str], set[str]]:
    """Read verified PEP 794 ownership from one installed distribution."""
    name, root, entries = package
    metadata_entries = [entry for entry in entries if _is_distribution_metadata(entry)]
    if not metadata_entries:
        return name, set(), set()
    if len(metadata_entries) != 1:
        raise RuntimeError(
            f"Python package '{name}' contains {len(metadata_entries)} metadata records"
        )
    entry = metadata_entries[0]
    relative = _manifest_relative_path(entry[0])
    metadata_file = root / relative
    if (
        len(entry) != 4
        or entry[1] != "file"
        or not metadata_file.is_file()
        or _file_digest(metadata_file) != str(entry[3])
    ):
        raise RuntimeError(f"Python package '{name}' does not match '{relative}'")
    return _metadata_imports(metadata_file, name)


def _metadata_imports(
    metadata_file: Path, expected_name: str | None = None
) -> tuple[str, set[str], set[str]]:
    """Return one distribution's verified PEP 794 ownership declarations."""
    metadata = Parser().parsestr(metadata_file.read_text(encoding="utf-8"))
    actual_name = metadata.get("Name")
    if not actual_name:
        raise RuntimeError(f"Python metadata '{metadata_file}' has no Name")
    name = _normalize_distribution_name(actual_name)
    if expected_name is not None and name != expected_name:
        raise RuntimeError(
            f"Python package '{expected_name}' contains metadata for {actual_name!r}"
        )
    exclusive = {
        parsed
        for value in metadata.get_all("Import-Name", [])
        if (parsed := _metadata_import_name(value, "Import-Name")) is not None
    }
    namespaces = {
        parsed
        for value in metadata.get_all("Import-Namespace", [])
        if (parsed := _metadata_import_name(value, "Import-Namespace")) is not None
    }
    ambiguous = sorted(exclusive & namespaces)
    if ambiguous:
        raise RuntimeError(
            f"Python package '{name}' claims import {ambiguous[0]!r} as both "
            "exclusive and namespace ownership"
        )
    return name, exclusive, namespaces


def _import_owners(
    packages: list[tuple[str, set[str], set[str]]],
) -> dict[str, dict[str, str | list[str]]]:
    """Validate PEP 794 collisions and return deterministic provenance."""
    exclusive: dict[str, str] = {}
    namespaces: dict[str, set[str]] = {}
    for owner, package_exclusive, package_namespaces in sorted(packages):
        for name in sorted(package_exclusive):
            conflicts = sorted(
                {
                    *namespaces.get(name, set()),
                    *([exclusive[name]] if name in exclusive else []),
                }
                - {owner}
            )
            if conflicts:
                raise RuntimeError(
                    f"Python import {name!r} is claimed by incompatible packages "
                    f"'{conflicts[0]}' and '{owner}'"
                )
            exclusive[name] = owner
        for name in sorted(package_namespaces):
            conflict = exclusive.get(name)
            if conflict is not None and conflict != owner:
                raise RuntimeError(
                    f"Python import {name!r} is claimed by incompatible packages "
                    f"'{conflict}' and '{owner}'"
                )
            namespaces.setdefault(name, set()).add(owner)
    return {
        name: (
            {"exclusive": exclusive[name]}
            if name in exclusive
            else {"namespace": sorted(namespaces[name])}
        )
        for name in sorted(exclusive.keys() | namespaces.keys())
    }


def _environment_imports(
    roots: list[Path],
) -> list[tuple[str, set[str], set[str]]]:
    """Read top-level core metadata across immutable environment layers."""
    packages = []
    for root in roots:
        environment = root.resolve()
        if not environment.is_dir():
            raise RuntimeError(f"Python environment '{root}' is not a directory")
        packages.extend(
            _metadata_imports(metadata)
            for metadata in sorted(environment.glob("*.dist-info/METADATA"))
        )
    names = [name for name, _, _ in packages]
    if len(names) != len(set(names)):
        duplicate = next(name for name in sorted(names) if names.count(name) > 1)
        raise RuntimeError(
            f"Python environment stack contains duplicate package '{duplicate}'"
        )
    return packages


def _write_environment_imports(args: argparse.Namespace) -> None:
    """Validate layered import ownership into one cacheable provenance output."""
    if not args.environment:
        raise ValueError("--environment is required for this action")
    owners = _import_owners(_environment_imports(args.environment))
    args.output.write_text(
        json.dumps(owners, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )


def _path_owners(
    packages: list[tuple[str, Path, list[list[object]]]],
) -> dict[str, list[str]]:
    """Return complete path ownership across distributions."""
    path_owners: dict[str, set[str]] = {}
    for name, _, entries in packages:
        for entry in entries:
            relative = _manifest_relative_path(entry[0])
            path_owners.setdefault(relative, set()).add(name)
    return {path: sorted(names) for path, names in path_owners.items()}


def _copy_overlay_file(
    output: Path,
    package: tuple[str, Path, list[list[object]]],
    entry: list[object],
    selected: dict[str, tuple[str, str, int]],
) -> None:
    """Copy one verified file with uv-compatible console-script precedence."""
    name, root, _ = package
    relative = _manifest_relative_path(entry[0])
    source = root / relative
    if not source.is_file() or _file_digest(source) != str(entry[3]):
        raise RuntimeError(f"Python package '{name}' does not match '{relative}'")
    destination = output / relative
    if destination.is_dir():
        raise RuntimeError(f"Python environment path '{relative}' changes kind")
    digest = str(entry[3])
    mode = entry[2]
    if not isinstance(mode, int):
        raise TypeError(f"Python package '{name}' has invalid mode for '{relative}'")
    previous = selected.get(relative)
    if previous is not None:
        previous_name, previous_digest, previous_mode = previous
        if (digest, mode) != (previous_digest, previous_mode):
            if relative.startswith("bin/") and "/" not in relative[4:]:
                return
            raise RuntimeError(
                f"Python environment file '{relative}' is owned by incompatible "
                f"packages '{previous_name}' and '{name}'"
            )
        return
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)
    destination.chmod(mode)
    selected[relative] = (name, digest, mode)


def _compose_environment(args: argparse.Namespace) -> None:
    """Materialize one import root while rejecting ambiguous ownership."""
    packages = _environment_packages(args)
    import_owners = _import_owners([_package_imports(package) for package in packages])
    path_owners = _path_owners(packages)
    output = args.output.resolve()
    output.mkdir()
    selected: dict[str, tuple[str, str, int]] = {}
    for package in packages:
        for entry in package[2]:
            relative = _manifest_relative_path(entry[0])
            destination = output / relative
            if entry[1] == "directory":
                if destination.is_file():
                    raise RuntimeError(
                        f"Python environment path '{relative}' changes kind"
                    )
                destination.mkdir(parents=True, exist_ok=True)
            elif entry[1] == "file":
                _copy_overlay_file(output, package, entry, selected)
            else:
                raise RuntimeError(f"Python package path '{relative}' has unknown kind")
    _validate_environment(output)
    provenance = {
        "format": 3,
        "imports": import_owners,
        "overlay": {path: owner for path, (owner, _, _) in selected.items()},
        "packages": [name for name, _, _ in packages],
        "paths": path_owners,
    }
    Path(_required(args.manifest, "--manifest")).write_text(
        json.dumps(provenance, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )


def _wheel_environment(
    args: argparse.Namespace,
    _process_environment: dict[str, str],
    _scratch: Path,
) -> None:
    """Install exact first-party wheels as a separately cacheable runtime layer."""
    packages = args.output.resolve()
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
    _install_wheels(wheels, packages)
    _normalize_entry_points(packages)
    _validate_environment(packages)
    _import_owners(_environment_imports([packages]))


def _project(
    args: argparse.Namespace, process_environment: dict[str, str], scratch: Path
) -> None:
    """Execute one first-party build, lint, or typecheck action."""
    source = Path(_required(args.source, "--source")).resolve()
    if args.mode == "wheel":
        copied_source = scratch / "source"
        # BSMR materializes source artifacts as relative symlinks into the input
        # tree. Dereference them into a private tree while retaining only the
        # executable modes a backend can observe; timestamps are not inputs.
        shutil.copytree(source, copied_source, copy_function=shutil.copy)
        timestamp = int(_SOURCE_DATE_EPOCH)
        for path in [copied_source, *copied_source.rglob("*")]:
            os.utime(path, (timestamp, timestamp), follow_symlinks=False)
        source = copied_source.resolve()
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
        distribution = _normalize_distribution_name(
            _required(args.distribution, "--distribution")
        )
        environments = _project_environments(args, process_environment)
        _configure_target_platform(args.python_platform, process_environment, scratch)
        _package_build_variables(
            args.package_build_variable, distribution, process_environment
        )
        reference, backend_paths = _backend_specification(project)
        settings = _config_settings(
            args.config_setting, args.package_config_setting, distribution
        )
        with (
            _backend_process(
                python,
                backend_paths,
                environments,
                process_environment,
                args.python_platform,
            ),
            contextlib.chdir(project),
        ):
            _build_wheel(_load_backend(reference), output, settings, distribution)
        return
    elif args.mode == "ruff":
        ruff = Path(_required(args.ruff, "--ruff")).resolve()
        command = [str(ruff), "check", "--no-cache", "--output-format", "concise"]
    else:
        environments = _project_environments(args, process_environment)
        ty = Path(_required(args.ty, "--ty")).resolve()
        command = [
            str(ty),
            "check",
            "--python",
            str(python),
            *(
                argument
                for environment in environments
                for argument in ("--extra-search-path", str(environment))
            ),
            "--output-format",
            "concise",
            "--no-progress",
            "--color",
            "never",
        ]
    _run(command, process_environment, cwd=project)
    output.write_text("ok\n", encoding="utf-8")


def main() -> None:
    """Dispatch one validated native Python action."""
    args = _arguments()
    if args.output.exists():
        raise FileExistsError(f"action output '{args.output}' already exists")
    scratch, process_environment = _state(args.output)
    if args.mode in ("locked-package", "wheel"):
        _configure_sysconfig(args.python, scratch, process_environment)
    if args.mode in ("install-wheel", "locked-package"):
        _locked_package(args, process_environment, scratch)
    elif args.mode == "select-package":
        _select_locked_package(args, process_environment, scratch)
    elif args.mode == "compose-environment":
        _compose_environment(args)
    elif args.mode == "validate-environments":
        _write_environment_imports(args)
    elif args.mode == "wheel-environment":
        _wheel_environment(args, process_environment, scratch)
    else:
        _project(args, process_environment, scratch)


if __name__ == "__main__":
    main()

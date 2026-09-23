# ===----------------------------------------------------------------------===
# Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc
# Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

"""
Run a crate's Cargo buildscript.
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any, IO, NamedTuple, Optional


IS_WINDOWS: bool = os.name == "nt"
TOOL_CWD: str = os.path.join(os.getcwd(), "")

# Sentinel used to mark OUT_DIR-relative paths emitted by buildscripts.
# We later replace this sentinel with the actual content-addressed path, once that is known.
OUT_DIR_SENTINEL: str = "${__BUILDSCRIPT_OUT_DIR__}"


def eprint(*args: Any, **kwargs: Any) -> None:
    print(*args, end="\n", file=sys.stderr, flush=True, **kwargs)


def cfg_env(rustc_cfg: Path) -> dict[str, str]:
    """Convert compiler cfgs to Cargo environment values, including empty flags."""
    with rustc_cfg.open(encoding="utf-8") as f:
        lines = f.readlines()

    cfgs: dict[str, list[str]] = {}
    for line in lines:
        key, separator, value = line.strip().partition("=")
        values = cfgs.setdefault("CARGO_CFG_" + key.upper().replace("-", "_"), [])
        if separator:
            values.append(value[1:-1])
    return {key: ",".join(values) for key, values in cfgs.items()}


def create_cwd(path: Path, manifest_dir: Path) -> Path:
    """Copy package sources into a self-contained cached output directory.

    Consumers compile this directory, including source changes made by the script.
    Excluding package toolchain files prevents Rustup probes from selecting a
    different compiler than the one declared by the build.
    """

    if path.is_symlink():
        path.unlink()
    elif path.exists():
        shutil.rmtree(path)
    path.mkdir()

    for dir_entry in manifest_dir.iterdir():
        if dir_entry.name not in ["rust-toolchain", "rust-toolchain.toml"]:
            destination = path.joinpath(dir_entry.name)
            if dir_entry.is_dir() and not dir_entry.is_symlink():
                shutil.copytree(dir_entry, destination, symlinks=True)
            else:
                shutil.copy2(dir_entry, destination, follow_symlinks=False)

    return path


# In some environments, invoking the rustc binary may actually invoke another
# tool that fetches the binary from a remote location. This fetch may encounter
# network errors. Ideally, build scripts that invoke rustc would reliably fail
# when such a thing happens, but in practice they don't. To mitigate, we
# manually invoke `rustc --version` and make sure that succeeds.
def ensure_rustc_available(
    env: dict[str, str],
    cwd: Path,
    target: str,
) -> None:
    rustc = env.get("RUSTC")
    assert rustc is not None, "RUSTC env is missing"

    # NOTE: `HOST` is optional.
    host = env.get("HOST")

    try:
        # Run through cmd.exe on Windows so if rustc is a batch script
        # (like the command_alias trampoline is), it is found relative to
        # cwd.
        #
        # Executing `os.path.join(cwd, rustc)` would also work, but because
        # of `../` in the path, it's possible to hit path length limits.
        # Resolving it would remove the `..` but then sometimes things
        # fail with exit code `3221225725` ("out of stack memory").
        # I suspect it's some infinite loop brought about by the trampoline
        # and symlinks.
        subprocess.check_output(  # noqa: P204
            [rustc, "--version"],
            cwd=cwd,
            shell=IS_WINDOWS,
        )
        # A multiplexed sysroot may involve another fetch,
        # so pass `--target` to check that too.
        if host != target:
            subprocess.check_output(  # noqa: P204
                [rustc, f"--target={target}", "--version"],
                cwd=cwd,
                shell=IS_WINDOWS,
            )
    except OSError as ex:
        eprint(f"Failed to run {rustc} because {ex}")
        sys.exit(1)
    except subprocess.CalledProcessError as ex:
        eprint(f"Command failed with exit code {ex.returncode}")
        eprint(f"Command: {ex.cmd}")
        if ex.stdout:
            eprint(f"Stdout: {ex.stdout}")
        sys.exit(1)


def run_buildscript(
    buildscript: str,
    env: dict[str, str],
    cwd: Path,
) -> str:
    try:
        return subprocess.check_output(
            os.path.abspath(buildscript),
            encoding="utf-8",
            env=env,
            cwd=cwd,
        )
    except OSError as ex:
        print(f"Failed to run {buildscript} because {ex}", file=sys.stderr)
        sys.exit(1)
    except subprocess.CalledProcessError as ex:
        sys.exit(ex.returncode)


class Args(NamedTuple):
    buildscript: str
    rustc_cfg: Path
    rustc_host_tuple: Optional[Path]
    manifest_dir: Path
    create_cwd: Path
    outfile: IO[str]
    rustc_link_lib: bool
    rustc_link_search: bool
    metadata_out: Path
    metadata_dependency: list[list[str]]


class Metadata(NamedTuple):
    """Keep ordered Cargo metadata attached to the producer's generated directories."""

    # Producer's private source directory.
    cwd: str
    # Producer's generated files directory.
    out_dir: str
    # Emission order decides normalized-key collisions.
    values: tuple[tuple[str, str], ...]

    @classmethod
    def read(cls, path: Path) -> 'Metadata':
        """Validate a cached record before exposing values to a dependent script."""
        record = json.loads(path.read_text(encoding='utf-8'))
        if not isinstance(record, dict) or set(record) != {'cwd', 'out_dir', 'values'}:
            raise ValueError(f'invalid build-script metadata: {path}')
        for field in ['cwd', 'out_dir']:
            if (
                not isinstance(record[field], str)
                or not Path(record[field]).is_absolute()
            ):
                raise ValueError(f'invalid metadata directory {field}: {path}')
        values = record['values']
        if not isinstance(values, list) or not all(
            isinstance(pair, list)
            and len(pair) == 2
            and all(isinstance(value, str) for value in pair)
            for pair in values
        ):
            raise ValueError(f'invalid metadata values: {path}')
        return cls(
            record['cwd'],
            record['out_dir'],
            tuple((key, value) for key, value in values),
        )

    @staticmethod
    def parse(line: str) -> tuple[str, str]:
        """Decode modern metadata or Cargo's unreserved legacy keys."""
        directive = line.split(":", 1)[1].lstrip(":").split("=", 1)[0]
        if line.startswith('cargo::metadata='):
            data = line.removeprefix('cargo::metadata=')
        elif not line.startswith('cargo::') and directive not in [
            'rustc-flags',
            'rustc-link-lib',
            'rustc-link-search',
            'rustc-link-arg-cdylib',
            'rustc-cdylib-link-arg',
            'rustc-link-arg-bins',
            'rustc-link-arg-bin',
            'rustc-link-arg-tests',
            'rustc-link-arg-benches',
            'rustc-link-arg-examples',
            'rustc-link-arg',
            'rustc-cfg',
            'rustc-check-cfg',
            'rustc-env',
        ]:
            # Cargo's original syntax treats unreserved keys as metadata.
            data = line.removeprefix('cargo:')
        else:
            sys.exit(f'unsupported build-script directive: {directive}')
        key, separator, value = data.partition('=')
        if not separator:
            sys.exit(f'invalid build-script metadata: {line}')
        return key, value

    def write(self, path: Path) -> None:
        """Publish the ordered values only after every directive was accepted."""
        path.write_text(json.dumps(self._asdict()) + '\n', encoding='utf-8')

    def environment(self, prefix: str, out_dir: Path, cwd: Path) -> dict[str, str]:
        """Bind cached paths to the current artifacts and preserve last-write ordering."""
        roots = {self.cwd: str(cwd.absolute()), self.out_dir: str(out_dir.absolute())}
        pattern = re.compile('|'.join(re.escape(root) for root in roots))
        environment = {}
        for key, value in self.values:
            name = f'{prefix}_{key}'.upper().replace('-', '_')
            environment[name] = pattern.sub(lambda match: roots[match.group()], value)
        return environment


def arg_parse() -> Args:
    parser = argparse.ArgumentParser(description="Run Rust build script")
    parser.add_argument("--buildscript", type=str, required=True)
    parser.add_argument("--rustc-cfg", type=Path, required=True)
    parser.add_argument("--rustc-host-tuple", type=Path)
    parser.add_argument("--manifest-dir", type=Path, required=True)
    parser.add_argument("--create-cwd", type=Path, required=True)
    parser.add_argument("--outfile", type=argparse.FileType("w"), required=True)
    parser.add_argument("--rustc-link-lib", action="store_true")
    parser.add_argument("--rustc-link-search", action="store_true")
    parser.add_argument("--metadata-out", type=Path, required=True)
    parser.add_argument("--metadata-dependency", nargs=4, action="append", default=[])

    return Args(**vars(parser.parse_args()))


def main() -> None:  # noqa: C901
    args = arg_parse()

    env = cfg_env(args.rustc_cfg)

    out_dir = os.getenv("OUT_DIR")
    assert out_dir is not None, "OUT_DIR env is missing"
    os.makedirs(out_dir, exist_ok=True)
    env["OUT_DIR"] = os.path.abspath(out_dir)

    cwd = create_cwd(args.create_cwd, args.manifest_dir)
    env["CARGO_MANIFEST_DIR"] = os.path.abspath(cwd)
    env["CARGO_MANIFEST_PATH"] = os.path.join(env["CARGO_MANIFEST_DIR"], "Cargo.toml")

    env = dict(os.environ, **env)
    for (
        prefix,
        metadata_path,
        dependency_out,
        dependency_cwd,
    ) in args.metadata_dependency:
        metadata = Metadata.read(Path(metadata_path))
        env.update(
            metadata.environment(prefix, Path(dependency_out), Path(dependency_cwd))
        )

    target = env.get("TARGET")
    if target is None:
        assert args.rustc_host_tuple, "TARGET env is missing"
        with args.rustc_host_tuple.open(encoding="utf-8") as f:
            target = f.read().strip()
            env["TARGET"] = target

    if os.sep in env.get("LD", ""):
        env["LD"] = os.path.abspath(env["LD"])
    if os.sep in env.get("CC", ""):
        env["CC"] = os.path.abspath(env["CC"])
    if os.sep in env.get("CXX", ""):
        env["CXX"] = os.path.abspath(env["CXX"])
    if os.sep in env.get("AR", ""):
        env["AR"] = os.path.abspath(env["AR"])

    ensure_rustc_available(
        env=env,
        cwd=cwd,
        target=target,
    )

    script_output = run_buildscript(args.buildscript, env=env, cwd=cwd)

    cargo_rustc_cfg_pattern = re.compile("^cargo::?rustc-(cfg|check-cfg)=(.*)")
    cargo_error_pattern = re.compile("^cargo::error=(.*)")
    cargo_rustc_env_pattern = re.compile("^cargo::?rustc-env=(.+?)=(.*)")
    cargo_rustc_link_lib_pattern = re.compile("^cargo::?rustc-link-lib=(.*)")
    cargo_rustc_link_search_pattern = re.compile(
        "^cargo::?rustc-link-search=([a-z]+=)?(.+)"
    )
    out_dir_abs = env["OUT_DIR"]

    # Rewrite a path inside OUT_DIR to OUT_DIR_SENTINEL; None if it is elsewhere.
    def reanchor_out_dir(path: str) -> Optional[str]:
        if path == out_dir_abs:
            return OUT_DIR_SENTINEL
        if path.startswith(out_dir_abs + os.sep):
            return OUT_DIR_SENTINEL + path[len(out_dir_abs) :]
        return None

    flags = ""
    metadata_values: list[tuple[str, str]] = []
    for line in script_output.split("\n"):
        line = line.strip()
        cargo_error_match = cargo_error_pattern.match(line)
        if cargo_error_match:
            sys.exit(f"build script error: {cargo_error_match.group(1)}")
        cargo_rustc_cfg_match = cargo_rustc_cfg_pattern.match(line)
        if cargo_rustc_cfg_match:
            flag, value = cargo_rustc_cfg_match.groups()
            flags += f"--{flag}={value}\n"
            continue
        cargo_rustc_env_match = cargo_rustc_env_pattern.match(line)
        if cargo_rustc_env_match:
            key = cargo_rustc_env_match.group(1)
            value = cargo_rustc_env_match.group(2)
            reanchored = reanchor_out_dir(value)
            if reanchored is not None:
                flags += f"--env-set={key}={reanchored}\n"
            elif value.startswith(TOOL_CWD):
                relative_path = value[len(TOOL_CWD) :]
                flags += f"--env-set={key}=$(abspath {relative_path})\n"
            else:
                flags += f"--env-set={key}={value}\n"
            continue
        cargo_rustc_link_lib_match = cargo_rustc_link_lib_pattern.match(line)
        if args.rustc_link_lib and cargo_rustc_link_lib_match:
            value = cargo_rustc_link_lib_match.group(1)
            flags += f"-l{value}\n"
            continue
        cargo_rustc_link_search_match = cargo_rustc_link_search_pattern.match(line)
        if args.rustc_link_search and cargo_rustc_link_search_match:
            kind = cargo_rustc_link_search_match.group(1) or ""
            path = cargo_rustc_link_search_match.group(2)
            reanchored = reanchor_out_dir(path)
            if reanchored is not None:
                flags += f"-L{kind}{reanchored}\n"
            elif path.startswith(TOOL_CWD):
                relative_path = path[len(TOOL_CWD) :]
                flags += f"-L{kind}$(abspath {relative_path})\n"
            else:
                sys.exit(f"build script link search is outside declared inputs: {path}")
            continue
        if line.startswith("cargo:"):
            directive = line.split(":", 1)[1].lstrip(":").split("=", 1)[0]
            if (
                directive in ["warning", "rerun-if-changed", "rerun-if-env-changed"]
                and '=' in line
            ):
                print(line)
                continue
            metadata_values.append(Metadata.parse(line))
            continue
        print(line, end="\n")
    args.outfile.write(flags)
    Metadata(env['CARGO_MANIFEST_DIR'], out_dir_abs, tuple(metadata_values)).write(
        args.metadata_out
    )


if __name__ == "__main__":
    main()

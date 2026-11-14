# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Resolve Debian packages with APT without installing into the selected image.

Run explicitly in a trusted Linux resolver environment containing python3-apt.
The root must be a pristine unpack of the base manifest named by --base-digest.
Only its package status and archive keyring enter the private resolver state.
Image APT configuration, hooks, credentials, and executables are not loaded.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import tempfile
from pathlib import Path
from typing import TYPE_CHECKING, TypedDict
from urllib.parse import urlsplit

if TYPE_CHECKING:
    import apt


class LockedPackage(TypedDict):
    """APT-selected package identity and verified acquisition inputs."""

    name: str
    version: str
    architecture: str
    url: str
    sha256: str
    size: int


class PackageLock(TypedDict):
    """A package transaction bound to an immutable base and explicit policy."""

    version: int
    base: str
    platform: str
    requested: list[str]
    install_recommends: bool
    packages: list[LockedPackage]


class ResolutionError(ValueError):
    """Reject a package plan that cannot be consumed without another resolution."""


def package_record(package: apt.Package) -> LockedPackage:
    """Retain only authenticated, pinned candidates selected by APT."""
    if package.marked_delete or package.marked_downgrade:
        raise ResolutionError(
            "DEBIAN_UNSUPPORTED_TRANSACTION: removals and downgrades are unsupported"
        )
    candidate = package.candidate
    if candidate is None or not candidate.downloadable:
        raise ResolutionError(
            "DEBIAN_MISSING_CANDIDATE: selected package has no downloadable version"
        )
    uri = urlsplit(candidate.uri)
    if uri.scheme not in ("https", "http") or uri.username or uri.password:
        raise ResolutionError(
            "DEBIAN_INVALID_ORIGIN: package needs a public HTTP(S) origin"
        )
    if not any(origin.trusted for origin in candidate.origins):
        raise ResolutionError(
            "DEBIAN_UNTRUSTED_ORIGIN: candidate has no authenticated repository"
        )
    if not re.fullmatch(r"[0-9a-f]{64}", candidate.sha256) or candidate.size <= 0:
        raise ResolutionError("DEBIAN_MISSING_DIGEST: candidate lacks SHA-256 or size")
    # Debian archive endpoints provide HTTPS for the authenticated HTTP index URLs.
    if uri.scheme == "http" and uri.hostname not in (
        "deb.debian.org",
        "security.debian.org",
        "snapshot.debian.org",
    ):
        raise ResolutionError("DEBIAN_INSECURE_ORIGIN: use HTTPS for this repository")
    url = uri._replace(scheme="https").geturl()
    return LockedPackage(
        name=package.shortname,
        version=candidate.version,
        architecture=candidate.architecture,
        url=url,
        sha256=candidate.sha256,
        size=candidate.size,
    )


def base_file(root: Path, relative: str) -> Path:
    """Keep package status and trust keys inside the declared unpacked base."""
    path = root / relative
    if (
        path.is_symlink()
        or not path.is_file()
        or not path.resolve().is_relative_to(root.resolve())
    ):
        raise ResolutionError(
            f"DEBIAN_INVALID_BASE_FILE: {relative} must be a regular file inside the base"
        )
    return path


def open_cache(args: argparse.Namespace, private: Path) -> apt.Cache:
    """Load authenticated indexes without inheriting resolver or image configuration."""
    if "APT_CONFIG" in os.environ:
        raise ResolutionError(
            "DEBIAN_AMBIENT_CONFIGURATION: unset APT_CONFIG before resolving packages"
        )
    try:
        import apt
        import apt_pkg
    except ModuleNotFoundError as error:
        raise ResolutionError(
            "DEBIAN_MISSING_RESOLVER: install python3-apt in the resolver environment"
        ) from error

    architecture = args.platform.removeprefix("linux/")
    for path in (
        "etc/apt/sources.list.d",
        "etc/apt/apt.conf.d",
        "var/lib/apt/lists/partial",
        "var/lib/dpkg",
        "var/cache/apt/archives/partial",
        "var/log/apt",
    ):
        (private / path).mkdir(parents=True, exist_ok=True)
    shutil.copyfile(
        base_file(args.root, "var/lib/dpkg/status"), private / "var/lib/dpkg/status"
    )
    keyring = private / "archive-keyring.gpg"
    shutil.copyfile(
        base_file(args.root, "usr/share/keyrings/debian-archive-keyring.gpg"), keyring
    )
    source = f"deb [signed-by={keyring}] {args.repository} {args.suite} main\n"
    (private / "etc/apt/sources.list").write_text(source)
    # Discard environment-specific policy and hooks before reading any base state.
    for key in list(apt_pkg.config.keys()):
        apt_pkg.config.clear(key)
    apt_pkg.config.set("Dir", str(private))
    apt_pkg.config.set("Dir::Etc", "etc/apt")
    apt_pkg.init_config()
    settings = {
        "Dir": str(private),
        "Dir::State": "var/lib/apt",
        "Dir::State::lists": "lists",
        "Dir::State::status": str(private / "var/lib/dpkg/status"),
        "Dir::Cache": "var/cache/apt",
        "Dir::Cache::archives": "archives",
        "Dir::Log": "var/log/apt",
        "Dir::Etc": "etc/apt",
        "Dir::Etc::sourcelist": "sources.list",
        "Dir::Etc::sourceparts": "sources.list.d",
        "Dir::Etc::parts": "apt.conf.d",
        "Dir::Bin::Methods": "/usr/lib/apt/methods",
        "Dir::Bin::dpkg": "/usr/bin/dpkg",
        "APT::Architecture": architecture,
        "APT::Architectures::": architecture,
        "APT::Install-Recommends": "false",
        "APT::Install-Suggests": "false",
        "APT::Get::AllowUnauthenticated": "false",
        "Acquire::AllowInsecureRepositories": "false",
    }
    for key, value in settings.items():
        apt_pkg.config.set(key, value)
    apt_pkg.init_system()
    cache = apt.Cache(memonly=True)
    cache.update(raise_on_error=True)
    cache.open(None)
    return cache


def resolve(args: argparse.Namespace, private: Path) -> PackageLock:
    """Resolve from copied base state using APT's dependency solver."""
    cache = open_cache(args, private)
    import apt

    resolver = apt.cache.ProblemResolver(cache)
    for name in args.packages:
        if name not in cache:
            raise ResolutionError(f"DEBIAN_UNKNOWN_PACKAGE: {name}")
        package = cache[name]
        package.mark_install(auto_fix=False, auto_inst=True, from_user=True)
        resolver.protect(package)
    resolver.resolve()
    if cache.broken_count or cache.delete_count:
        raise ResolutionError(
            "DEBIAN_UNSATISFIED_TRANSACTION: dependencies cannot be installed without removals"
        )
    packages = sorted(
        (package_record(package) for package in cache.get_changes()),
        key=lambda package: (package["name"], package["architecture"]),
    )
    return PackageLock(
        version=1,
        base=args.base_digest,
        platform=args.platform,
        requested=sorted(args.packages),
        install_recommends=False,
        packages=packages,
    )


def main() -> None:
    """Write a new lock only after the whole authenticated resolution succeeds."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--base-digest", required=True)
    parser.add_argument(
        "--platform", choices=("linux/amd64", "linux/arm64"), required=True
    )
    parser.add_argument("--repository", required=True)
    parser.add_argument("--suite", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("packages", nargs="+")
    args = parser.parse_args()
    if not re.fullmatch(r"sha256:[0-9a-f]{64}", args.base_digest):
        parser.error("base-digest must be a lowercase SHA-256 manifest digest")
    if not re.fullmatch(r"[a-z0-9][a-z0-9+.-]*", args.suite):
        parser.error("suite must be one Debian suite name")
    repository = urlsplit(args.repository)
    if (
        repository.scheme not in ("https", "http")
        or not repository.hostname
        or repository.username
        or repository.password
        or any(c.isspace() for c in args.repository)
    ):
        parser.error("repository must be one credential-free HTTP(S) URL")
    if len(set(args.packages)) != len(args.packages) or any(
        not re.fullmatch(r"[a-z0-9][a-z0-9+.-]*", name) for name in args.packages
    ):
        parser.error("packages must be unique Debian package names")
    status = base_file(args.root, "var/lib/dpkg/status")
    before = hashlib.sha256(status.read_bytes()).hexdigest()
    with tempfile.TemporaryDirectory(prefix="bsmr-apt-resolve-") as scratch:
        lock = resolve(args, Path(scratch))
    if hashlib.sha256(status.read_bytes()).hexdigest() != before:
        raise ResolutionError(
            "DEBIAN_BASE_CHANGED: package resolution modified the source status"
        )
    with args.output.open("x") as output:
        json.dump(lock, output, indent=2)
        output.write("\n")


if __name__ == "__main__":
    main()

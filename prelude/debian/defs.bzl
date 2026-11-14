# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Acquires an APT-resolved package set bound to one immutable base image.

load("@prelude//oci:providers.bzl", "OciImageInfo")

def _record(value, fields: list[str], name: str) -> None:
    """Reject unknown lock fields so a changed policy cannot be silently ignored."""
    if type(value) != "dict" or sorted(value.keys()) != sorted(fields):
        fail("DEBIAN_INVALID_LOCK: {} has an unsupported schema".format(name))

def _text(value, name: str) -> None:
    """Require process-safe nonempty metadata before constructing download actions."""
    if type(value) != "string" or value == "" or "\x00" in value or "\n" in value or "\r" in value:
        fail("DEBIAN_INVALID_LOCK: {} must be a nonempty string".format(name))

def _sha256(value) -> None:
    """Require the exact digest representation consumed by the build cache."""
    if type(value) != "string" or len(value) != 64 or any([c not in "0123456789abcdef" for c in value.elems()]):
        fail("DEBIAN_INVALID_LOCK: sha256 must contain 64 lowercase hexadecimal characters")

def _deb_packages_impl(ctx: AnalysisContext) -> list[Provider]:
    """Download verified archive bytes; installation remains a separate image action."""
    base = ctx.attrs.base[OciImageInfo]
    requested = sorted(ctx.attrs.packages)
    if not requested or len({name: True for name in requested}) != len(requested):
        fail("DEBIAN_INVALID_REQUEST: packages must be nonempty and unique")
    for name in requested:
        _text(name, "requested package")
    directory = ctx.actions.declare_output("packages", dir = True)
    lock = ctx.attrs.lock

    def download(inner, artifacts, outputs):
        """Bind downloaded packages to the exact image and resolution policy."""
        value = artifacts[lock].read_json()
        _record(value, ["version", "base", "platform", "requested", "install_recommends", "packages"], "lock")
        if value["version"] != 1:
            fail("DEBIAN_INVALID_LOCK: unsupported lock version")
        if value["base"] != artifacts[base.descriptor].read_json()["digest"] or value["platform"] != base.platform:
            fail("DEBIAN_BASE_MISMATCH: lock belongs to another base image or platform")
        if value["requested"] != requested or value["install_recommends"] != False:
            fail("DEBIAN_REQUEST_MISMATCH: regenerate the lock for the requested packages without recommends")
        if type(value["packages"]) != "list":
            fail("DEBIAN_INVALID_LOCK: packages must be a list")
        files = {}
        identities = {}
        for package in value["packages"]:
            _record(package, ["name", "version", "architecture", "url", "sha256", "size"], "package")
            for field in ["name", "version", "architecture", "url"]:
                _text(package[field], field)
            _sha256(package["sha256"])
            if package["architecture"] not in ["all", base.platform.split("/")[1]]:
                fail("DEBIAN_PLATFORM_MISMATCH: package architecture differs from the image")
            if type(package["size"]) != "int" or package["size"] <= 0:
                fail("DEBIAN_INVALID_LOCK: package size must be positive")
            url = package["url"]
            if not url.startswith("https://") or "@" in url or "#" in url or " " in url:
                fail("DEBIAN_INVALID_LOCK: package URL must be credential-free HTTPS")
            identity = (package["name"], package["architecture"])
            if identity in identities or package["sha256"] in files:
                fail("DEBIAN_INVALID_LOCK: duplicate package identity or archive")
            identities[identity] = True
            files[package["sha256"]] = inner.actions.download_file(
                package["sha256"] + ".deb", url,
                sha256 = package["sha256"], size_bytes = package["size"],
            )
        inner.actions.copied_dir(outputs[directory], {sha + ".deb": file for sha, file in files.items()})

    ctx.actions.dynamic_output(dynamic = [lock, base.descriptor], inputs = [], outputs = [directory.as_output()], f = download)
    return [DefaultInfo(default_output = directory)]

deb_packages = rule(impl = _deb_packages_impl, attrs = {
    "base": attrs.dep(providers = [OciImageInfo]),
    "packages": attrs.list(attrs.string()),
    "lock": attrs.source(),
}, doc = "Acquires an APT-resolved, checksum-pinned package set for the selected base; never installs or refreshes the lock.")

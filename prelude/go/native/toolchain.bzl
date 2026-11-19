# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Declares the official Go SDK that `.bsmr-go-toolchain.json` pins.

load("@prelude//:prelude.bzl", "native")
load("@prelude//toolchains/go:go_bootstrap_toolchain.bzl", "go_bootstrap_distr", "go_bootstrap_toolchain")
load("@prelude//toolchains/go:go_toolchain.bzl", "go_distr", "go_toolchain")

# `bsmr go toolchain` installs the host SDK and bootstrap wrapper beside these rules, in
# the directory of the `toolchains//` package, so the paths are the same in every layout.
_SDK = ".bsmr-go-sdk"
_WRAPPER = ".bsmr-go-tools/go_wrapper"

def _prebuilt_tool_impl(ctx):
    return [DefaultInfo(default_output = ctx.attrs.binary), RunInfo(args = [ctx.attrs.binary])]

_prebuilt_tool = rule(
    impl = _prebuilt_tool_impl,
    attrs = {"binary": attrs.source()},
)

def _unacquired_impl(_ctx):
    fail("the Go SDK pinned by .bsmr-go-toolchain.json is not acquired; run `bsmr go toolchain`")

# Stands in for `go` and `go_bootstrap` until acquisition, so the package that also holds
# the other languages' toolchains evaluates, and only a Go build reports the missing SDK.
_unacquired = rule(
    impl = _unacquired_impl,
    attrs = {},
    is_toolchain_rule = True,
)

def _by_execution_host(values):
    """Selects the value for the host that runs the SDK's tools, keyed by Go's `os-arch`."""
    return select({
        "config//os:linux": select({
            "config//cpu:arm64": values["linux-arm64"],
            "config//cpu:x86_64": values["linux-amd64"],
        }),
        "config//os:macos": select({
            "config//cpu:arm64": values["darwin-arm64"],
            "config//cpu:x86_64": values["darwin-amd64"],
        }),
    })

def _target_goos():
    """Selects `GOOS` from the platform of the artifact being built, not the host running it."""
    return select({"config//os:linux": "linux", "config//os:macos": "darwin"})

def _target_goarch():
    """Selects `GOARCH` from the platform of the artifact being built, not the host running it."""
    return select({"config//cpu:arm64": "arm64", "config//cpu:x86_64": "amd64"})

def native_go_toolchains(lock: str, acquired: bool):
    """Declares `go`, `go_bootstrap`, and the verified archive `bsmr go toolchain` installs.

    Args:
        lock: The contents of `.bsmr-go-toolchain.json`.
        acquired: Whether the host SDK and bootstrap wrapper exist in this package.
    """
    lock = json.decode(lock)
    if lock["generated_by"] != "bsmr go toolchain" or lock["schema"] != 1:
        fail(".bsmr-go-toolchain.json has an unsupported ownership marker or schema")
    archives = {"{}-{}".format(archive["os"], archive["arch"]): archive for archive in lock["archives"]}
    native.http_archive(
        name = "go_sdk_archive",
        sha256 = _by_execution_host({host: archive["sha256"] for host, archive in archives.items()}),
        size_bytes = _by_execution_host({host: archive["size"] for host, archive in archives.items()}),
        strip_prefix = "go",
        urls = [_by_execution_host({host: "https://go.dev/dl/" + archive["filename"] for host, archive in archives.items()})],
        visibility = ["PUBLIC"],
    )
    if not acquired:
        _unacquired(name = "go_bootstrap", visibility = ["PUBLIC"])
        _unacquired(name = "go", visibility = ["PUBLIC"])
        return
    host = _by_execution_host({host: tuple(host.split("-")) for host in archives})
    _prebuilt_tool(name = "go_bootstrap_wrapper", binary = _WRAPPER)
    go_bootstrap_distr(name = "go_bootstrap_distr", go_os_arch = host, go_root = _SDK)
    go_bootstrap_toolchain(
        name = "go_bootstrap",
        env_go_arch = _target_goarch(),
        env_go_os = _target_goos(),
        go_bootstrap_distr = ":go_bootstrap_distr",
        go_wrapper = ":go_bootstrap_wrapper",
        allow_local_cache_upload = True,
        visibility = ["PUBLIC"],
    )
    go_distr(name = "go_distr", go_os_arch = host, go_root = _SDK, version = lock["version"])
    go_toolchain(
        name = "go",
        env_go_arch = _target_goarch(),
        env_go_os = _target_goos(),
        env_go_experiment = ["none"],
        go_distr = ":go_distr",
        allow_local_cache_upload = True,
        visibility = ["PUBLIC"],
    )

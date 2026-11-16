# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Declares the official Go SDK that `.bsmr-go-toolchain.json` pins.

load("@prelude//:prelude.bzl", "native")
load("@prelude//go_bootstrap:go_bootstrap.bzl", "GoBootstrapDistrInfo")
load("@prelude//toolchains/go:go_bootstrap_toolchain.bzl", "go_bootstrap_distr", "go_bootstrap_toolchain")
load("@prelude//toolchains/go:go_toolchain.bzl", "go_distr", "go_toolchain")

_SDK = ":go_sdk_archive"

# Go requires absolute cache and root paths, while actions receive project-relative ones.
_COMPILE_WRAPPER = 'scratch="$(cd "$BSMR_SCRATCH_PATH" && pwd)" && GOROOT="$(cd "$2" && pwd)" GOCACHE="$scratch" TMPDIR="$scratch" exec "$1" build -trimpath -buildvcs=false -o "$3" "$4"'

def _bootstrap_wrapper_impl(ctx):
    """Compiles the bootstrap wrapper with the locked SDK, which no wrapper runs yet."""
    distr = ctx.attrs.go_bootstrap_distr[GoBootstrapDistrInfo]
    go_os, go_arch = ctx.attrs.go_os_arch
    output = ctx.actions.declare_output("go_wrapper", has_content_based_path = True)
    ctx.actions.run(
        cmd_args("/bin/sh", "-c", _COMPILE_WRAPPER, "go_wrapper", distr.bin_go, distr.go_root, output.as_output(), ctx.attrs.src),
        env = {"CGO_ENABLED": "0", "GOARCH": go_arch, "GOENV": "off", "GOEXPERIMENT": "", "GOFLAGS": "", "GOOS": go_os, "GOTOOLCHAIN": "local"},
        category = "go_bootstrap_wrapper",
        allow_local_cache_upload = True,
    )
    return [DefaultInfo(default_output = output), RunInfo(args = [output])]

_bootstrap_wrapper = rule(
    impl = _bootstrap_wrapper_impl,
    attrs = {
        "go_bootstrap_distr": attrs.exec_dep(providers = [GoBootstrapDistrInfo]),
        "go_os_arch": attrs.tuple(attrs.string(), attrs.string()),
        "src": attrs.source(default = "prelude//go_bootstrap/tools:go_wrapper.go"),
    },
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

def native_go_toolchains(lock: str):
    """Declares `go`, `go_bootstrap`, and the verified archive both build with.

    Args:
        lock: The contents of `.bsmr-go-toolchain.json`.
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
    host = _by_execution_host({host: tuple(host.split("-")) for host in archives})
    go_bootstrap_distr(name = "go_bootstrap_distr", go_os_arch = host, go_root = _SDK)
    _bootstrap_wrapper(name = "go_bootstrap_wrapper", go_bootstrap_distr = ":go_bootstrap_distr", go_os_arch = host)
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

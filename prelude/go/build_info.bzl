# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Renders the `modinfo` line that gives a linked Go binary its `debug.BuildInfo`.
#
# cmd/go writes `modinfo %q` into the link importcfg (cmd/go/internal/work/exec.go,
# writeLinkImportcfg), and cmd/link stores the unquoted bytes as `runtime.modinfo`.
# The text is `debug.BuildInfo.String()`, framed by the markers from
# cmd/go/internal/modload/build.go that `runtime/debug.ReadBuildInfo` strips.
#
# Settings follow setBuildInfo in cmd/go/internal/load/pkg.go, keeping its order
# and only the ones that are a pure function of the configuration. VCS stamping
# (vcs.revision, vcs.time, vcs.modified) is omitted: it would make link outputs
# depend on repository state outside the action's inputs. `-trimpath=true` is
# recorded because compilation passes `-trimpath %cwd%` (package_builder.bzl),
# so no host path enters the binary; cmd/go likewise drops `-ldflags` and the
# CGO_*FLAGS settings under -trimpath because they carry host paths.
# DefaultGODEBUG is omitted until the link applies `runtime.godebugDefault`.

load(":toolchain.bzl", "GoToolchainInfo")

# The framing bytes as Go escapes, ready to sit inside the quoted `modinfo` value.
_START = "\\x30\\x77\\xaf\\x0c\\x92\\x74\\x08\\x02\\x41\\xe1\\xc1\\x07\\xe6\\xd6\\x18\\xe6"
_END = "\\xf9\\x32\\x43\\x31\\x86\\x18\\x20\\x72\\x00\\x82\\x42\\x10\\x41\\x16\\xd8\\xf2"

# cmd/go records the microarchitecture level even when the environment leaves it
# unset; these are the defaults the official SDK builds with (internal/buildcfg).
_ARCH_LEVELS = {
    "amd64": ("GOAMD64", "v1"),
    "arm64": ("GOARM64", "v8.0"),
}

def go_modinfo(
        go_toolchain: GoToolchainInfo,
        package_lines: list[str],
        build_mode: str,
        build_tags: list[str],
        cgo_enabled: bool) -> str:
    """Return the importcfg `modinfo` line for a main package's `path`, `mod`, and `dep` lines."""
    settings = []
    if go_toolchain.asan:
        settings.append(("-asan", "true"))
    settings.append(("-buildmode", build_mode))
    settings.append(("-compiler", "gc"))
    if go_toolchain.race:
        settings.append(("-race", "true"))
    if build_tags:
        settings.append(("-tags", ",".join(build_tags)))
    settings.append(("-trimpath", "true"))
    settings.append(("CGO_ENABLED", "1" if cgo_enabled else "0"))
    settings.append(("GOARCH", go_toolchain.env_go_arch))
    if go_toolchain.env_go_experiment:
        settings.append(("GOEXPERIMENT", ",".join(go_toolchain.env_go_experiment)))
    settings.append(("GOOS", go_toolchain.env_go_os))
    if go_toolchain.env_go_arch == "arm" and go_toolchain.env_go_arm != None:
        settings.append(("GOARM", go_toolchain.env_go_arm))
    elif go_toolchain.env_go_arch in _ARCH_LEVELS:
        settings.append(_ARCH_LEVELS[go_toolchain.env_go_arch])

    lines = package_lines + ["build\t{}={}".format(key, value) for key, value in settings]
    info = "".join([line + "\n" for line in lines])
    escaped = info.replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", "\\n").replace("\t", "\\t")
    return "modinfo \"" + _START + escaped + _END + "\""

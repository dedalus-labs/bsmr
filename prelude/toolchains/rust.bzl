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

load(
    "@prelude//rust:rust_toolchain.bzl",
    "PanicRuntime",
    "RustReleaseChannel",
    "RustToolchainInfo",
    "rust_toolchain_configuration_error",
)

_DEFAULT_TRIPLE = select({
    "prelude//os:linux": select({
        "prelude//cpu:arm64": "aarch64-unknown-linux-gnu",
        "prelude//cpu:riscv64": "riscv64gc-unknown-linux-gnu",
        "prelude//cpu:x86_64": "x86_64-unknown-linux-gnu",
    }),
    "prelude//os:macos": select({
        "prelude//cpu:arm64": "aarch64-apple-darwin",
        "prelude//cpu:x86_64": "x86_64-apple-darwin",
    }),
    "prelude//os:windows": select({
        "prelude//cpu:arm64": select({
            # Rustup's default ABI for the host on Windows is MSVC, not GNU.
            # When you do `rustup install stable` that's the one you get. It
            # makes you opt in to GNU by `rustup install stable-gnu`.
            "DEFAULT": "aarch64-pc-windows-msvc",
            "prelude//abi:gnu": "aarch64-pc-windows-gnu",
            "prelude//abi:msvc": "aarch64-pc-windows-msvc",
        }),
        "prelude//cpu:x86_64": select({
            "DEFAULT": "x86_64-pc-windows-msvc",
            "prelude//abi:gnu": "x86_64-pc-windows-gnu",
            "prelude//abi:msvc": "x86_64-pc-windows-msvc",
        }),
    }),
})

def _system_rust_toolchain_impl(ctx):
    release_channel = RustReleaseChannel(ctx.attrs.release_channel)
    configuration_error = rust_toolchain_configuration_error(
        release_channel,
        ctx.attrs.nightly_features,
        ctx.attrs.codegen_backend != None,
    )
    if configuration_error:
        fail(configuration_error)

    return [
        DefaultInfo(),
        RustToolchainInfo(
            allow_lints = ctx.attrs.allow_lints,
            clippy_driver = RunInfo(args = ["clippy-driver"]),
            clippy_toml = ctx.attrs.clippy_toml[DefaultInfo].default_outputs[0] if ctx.attrs.clippy_toml else None,
            codegen_backend = ctx.attrs.codegen_backend,
            compiler = RunInfo(args = ["rustc"]),
            default_edition = ctx.attrs.default_edition,
            panic_runtime = PanicRuntime("unwind"),
            deny_lints = ctx.attrs.deny_lints,
            doctests = ctx.attrs.doctests,
            nightly_features = ctx.attrs.nightly_features,
            release_channel = release_channel.value,
            report_unused_deps = ctx.attrs.report_unused_deps,
            rustc_binary_flags = ctx.attrs.rustc_binary_flags,
            rustc_flags = ctx.attrs.rustc_flags,
            rustc_target_triple = ctx.attrs.rustc_target_triple,
            rustc_test_flags = ctx.attrs.rustc_test_flags,
            rustdoc = RunInfo(args = ["rustdoc"]),
            rustdoc_flags = ctx.attrs.rustdoc_flags,
            warn_lints = ctx.attrs.warn_lints,
        ),
    ]

system_rust_toolchain = rule(
    impl = _system_rust_toolchain_impl,
    attrs = {
        "allow_lints": attrs.list(attrs.string(), default = []),
        "clippy_toml": attrs.option(attrs.dep(providers = [DefaultInfo]), default = None),
        "codegen_backend": attrs.option(attrs.source(), default = None),
        "default_edition": attrs.option(attrs.string(), default = None),
        "deny_lints": attrs.list(attrs.string(), default = []),
        "doctests": attrs.bool(default = False),
        "nightly_features": attrs.bool(default = False),
        "release_channel": attrs.enum(RustReleaseChannel.values(), default = "stable"),
        "report_unused_deps": attrs.bool(default = False),
        "rustc_binary_flags": attrs.list(attrs.arg(), default = []),
        "rustc_flags": attrs.list(attrs.arg(), default = []),
        "rustc_target_triple": attrs.string(default = _DEFAULT_TRIPLE),
        "rustc_test_flags": attrs.list(attrs.arg(), default = []),
        "rustdoc_flags": attrs.list(attrs.arg(), default = []),
        "warn_lints": attrs.list(attrs.string(), default = []),
    },
    is_toolchain_rule = True,
)

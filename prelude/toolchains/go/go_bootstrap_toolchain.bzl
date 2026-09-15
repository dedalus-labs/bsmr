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

load("@prelude//go_bootstrap:go_bootstrap.bzl", "GoBootstrapDistrInfo", "GoBootstrapToolchainInfo")

def _go_bootstrap_toolchain_impl(ctx):
    """Use declared SDK and wrapper artifacts for cacheable bootstrap commands."""
    go_bootstrap_distr = ctx.attrs.go_bootstrap_distr[GoBootstrapDistrInfo]
    return [
        DefaultInfo(),
        GoBootstrapToolchainInfo(
            allow_local_cache_upload = ctx.attrs.allow_local_cache_upload,
            go_wrapper = ctx.attrs.go_wrapper[RunInfo],
            go = go_bootstrap_distr.bin_go,
            env_go_arch = ctx.attrs.env_go_arch,
            env_go_os = ctx.attrs.env_go_os,
            env_go_root = go_bootstrap_distr.go_root,
        ),
    ]

go_bootstrap_toolchain = rule(
    impl = _go_bootstrap_toolchain_impl,
    is_toolchain_rule = True,
    attrs = {
        "allow_local_cache_upload": attrs.bool(default = False, doc = "Require declared SDK and bootstrap executable artifacts; system Python is not eligible."),
        "env_go_arch": attrs.string(),
        "env_go_os": attrs.string(),
        "go_bootstrap_distr": attrs.exec_dep(providers = [GoBootstrapDistrInfo]),
        "go_wrapper": attrs.exec_dep(providers = [RunInfo], default = "prelude//go_bootstrap/tools:go_wrapper_py"),
    },
)

def _go_bootstrap_distr_impl(ctx):
    go_os, go_arch = ctx.attrs.go_os_arch
    suffix = ".exe" if go_os == "windows" else ""
    go_platform = go_os + "_" + go_arch
    bin_prefix = "bin/{}".format(go_platform) if ctx.attrs.multiplatform else "bin"
    return [
        DefaultInfo(),
        GoBootstrapDistrInfo(
            bin_go = RunInfo(ctx.attrs.go_root.project(bin_prefix + "/go" + suffix)),
            go_root = ctx.attrs.go_root,
        ),
    ]

go_bootstrap_distr = rule(
    impl = _go_bootstrap_distr_impl,
    attrs = {
        "go_os_arch": attrs.tuple(attrs.string(), attrs.string()),
        "go_root": attrs.source(allow_directory = True),
        "multiplatform": attrs.bool(default = False),
    },
)

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

load("@prelude//decls:common.bzl", "bsmr")
load("@prelude//decls:test_common.bzl", "test_common")
load(":julia_binary.bzl", "julia_binary_impl")
load(":julia_library.bzl", "julia_jll_library_impl", "julia_library_impl")
load(":julia_test.bzl", "julia_test_impl")
load(":julia_toolchain.bzl", "julia_toolchain")

implemented_rules = {
    "julia_binary": julia_binary_impl,
    "julia_jll_library": julia_jll_library_impl,
    "julia_library": julia_library_impl,
    "julia_test": julia_test_impl,
}

extra_attributes = {
    "julia_binary": {
        "deps": attrs.list(attrs.dep(), default = []),
        "julia_args": attrs.list(attrs.string(), default = []),
        "julia_flags": attrs.list(attrs.string(), default = []),
        "main": attrs.string(),
        "srcs": attrs.list(attrs.source(), default = []),
        "_julia_toolchain": julia_toolchain(),
    }
    | bsmr.labels_arg()
    | bsmr.contacts_arg(),
    "julia_jll_library": {
        "jll_name": attrs.string(),
        "lib_mapping": attrs.named_set(attrs.dep()),
        "uuid": attrs.string(),
        "_julia_toolchain": julia_toolchain(),
    }
    | bsmr.labels_arg()
    | bsmr.contacts_arg(),
    "julia_library": {
        "deps": attrs.list(attrs.dep(), default = []),
        "project_toml": attrs.source(),
        "resources": attrs.list(attrs.source(allow_directory = True), default = []),
        "srcs": attrs.list(attrs.source(), default = []),
        "_julia_toolchain": julia_toolchain(),
    }
    | bsmr.labels_arg()
    | bsmr.contacts_arg(),
    "julia_test": {
        "deps": attrs.list(attrs.dep(), default = []),
        "julia_args": attrs.list(attrs.string(), default = []),
        "julia_flags": attrs.list(attrs.string(), default = []),
        "main": attrs.string(),
        "srcs": attrs.list(attrs.source(), default = []),
        "_julia_toolchain": julia_toolchain(),
        # TODO: coverage
    }
    | bsmr.labels_arg()
    | bsmr.contacts_arg()
    | bsmr.inject_test_env_arg()
    | test_common.attributes(),
}

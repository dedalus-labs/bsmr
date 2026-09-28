# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Declares the host tools that every native frontend shares in `toolchains//`.

load("@prelude//tests:test_toolchain.bzl", "noop_test_toolchain")
load("@prelude//toolchains:cxx.bzl", "system_cxx_toolchain")
load("@prelude//toolchains:genrule.bzl", "system_genrule_toolchain")
load("@prelude//toolchains:python.bzl", "system_python_bootstrap_toolchain")
load("@prelude//toolchains:remote_test_execution.bzl", "remote_test_execution_toolchain")

def native_tools():
    """Declare the local bootstrap tools used by native frontends and custom recipes."""
    cxx = {"compiler": "gcc", "cxx_compiler": "g++", "compiler_type": "gcc", "linker": "g++"} if host_info().os.is_linux else {}
    system_cxx_toolchain(name = "cxx", visibility = ["PUBLIC"], **cxx)
    system_python_bootstrap_toolchain(name = "python_bootstrap", visibility = ["PUBLIC"])
    system_genrule_toolchain(name = "genrule", visibility = ["PUBLIC"])
    remote_test_execution_toolchain(name = "remote_test_execution", visibility = ["PUBLIC"])
    noop_test_toolchain(name = "test", visibility = ["PUBLIC"])

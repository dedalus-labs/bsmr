# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Supplies Rust's verified compiler and standard library to the existing rules.

load("@prelude//rust:rust_toolchain.bzl", "PanicRuntime", "RustToolchainInfo")

load("@prelude//toolchains:cxx.bzl", "system_cxx_toolchain")
load("@prelude//toolchains:python.bzl", "system_python_bootstrap_toolchain")
load("@prelude//toolchains:genrule.bzl", "system_genrule_toolchain")
load("@prelude//toolchains:remote_test_execution.bzl", "remote_test_execution_toolchain")
load("@prelude//tests:test_toolchain.bzl", "noop_test_toolchain")

def native_rust_tools(triple):
    """Declare the local bootstrap tools used by native Rust and custom recipes."""
    cxx = {"compiler": "gcc", "cxx_compiler": "g++", "compiler_type": "gcc", "linker": "g++"} if triple.endswith("linux-gnu") else {}
    system_cxx_toolchain(name = "cxx", visibility = ["PUBLIC"], **cxx)
    system_python_bootstrap_toolchain(name = "python_bootstrap", visibility = ["PUBLIC"])
    system_genrule_toolchain(name = "genrule", visibility = ["PUBLIC"])
    remote_test_execution_toolchain(name = "remote_test_execution", visibility = ["PUBLIC"])
    noop_test_toolchain(name = "test", visibility = ["PUBLIC"])

def _toolchain(ctx):
    """Keep the compiler tree and sysroot in every consuming action's inputs."""
    compiler = ctx.attrs.compiler[DefaultInfo].default_outputs[0]
    clippy = ctx.attrs.clippy[DefaultInfo].default_outputs[0]
    library_path = "DYLD_LIBRARY_PATH" if ctx.attrs.triple.endswith("apple-darwin") else "LD_LIBRARY_PATH"
    standard = ctx.attrs.standard_library[DefaultInfo].default_outputs[0]
    return [DefaultInfo(), RustToolchainInfo(
        compiler = RunInfo(args = cmd_args("/usr/bin/env", cmd_args(compiler.project("lib"), format = library_path + "={}"), compiler.project("bin/rustc"), hidden = [compiler])),
        clippy_driver = RunInfo(args = cmd_args("/usr/bin/env", cmd_args(compiler.project("lib"), format = library_path + "={}"), clippy.project("bin/clippy-driver"), hidden = [compiler, clippy])),
        rustdoc = RunInfo(args = cmd_args("/usr/bin/env", cmd_args(compiler.project("lib"), format = library_path + "={}"), compiler.project("bin/rustdoc"), hidden = [compiler])),
        sysroot_path = standard,
        rustc_target_triple = ctx.attrs.triple,
        nightly_features = ctx.attrs.nightly_features,
        panic_runtime = PanicRuntime("unwind"),
    )]

native_rust_toolchain = rule(
    impl = _toolchain,
    attrs = {
        "compiler": attrs.dep(),
        "clippy": attrs.dep(),
        "standard_library": attrs.dep(),
        "triple": attrs.string(),
        "nightly_features": attrs.bool(default = False),
    },
    is_toolchain_rule = True,
)

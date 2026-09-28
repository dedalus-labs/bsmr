# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Validates linker selection without requiring host tool binaries.

load("@prelude//:asserts.bzl", "asserts")
load("@prelude//os_lookup:defs.bzl", "Os")
load("@prelude//toolchains:cxx.bzl", "default_linker_flags")

def test_default_linker_flags():
    """The bundled lld default applies only to Clang linker drivers on Linux."""
    asserts.equals(["-fuse-ld=lld"], default_linker_flags(Os("linux"), "clang"))
    asserts.equals(["-fuse-ld=lld"], default_linker_flags(Os("linux"), "clang++"))
    asserts.equals([], default_linker_flags(Os("linux"), "wild"))
    asserts.equals([], default_linker_flags(Os("linux"), "g++"))
    asserts.equals([], default_linker_flags(Os("macos"), "clang++"))

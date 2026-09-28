# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Validates Rust release-channel policy independently of toolchain acquisition.

load("@prelude//:asserts.bzl", "asserts")
load(
    "@prelude//rust:rust_toolchain.bzl",
    "RustReleaseChannel",
    "rust_toolchain_configuration_error",
)

def test_rust_toolchain_configuration():
    """Stable defaults stay boring while experimental features require nightly."""
    stable = RustReleaseChannel("stable")
    nightly = RustReleaseChannel("nightly")

    asserts.equals(None, rust_toolchain_configuration_error(stable, False, False))
    asserts.equals(None, rust_toolchain_configuration_error(nightly, False, False))
    asserts.equals(None, rust_toolchain_configuration_error(nightly, True, True))
    asserts.equals(
        "Rust nightly features require release_channel = \"nightly\"",
        rust_toolchain_configuration_error(stable, True, False),
    )
    asserts.equals(
        "Rust codegen backends require release_channel = \"nightly\"",
        rust_toolchain_configuration_error(stable, False, True),
    )

---
id: rust_toolchains
title: Rust Toolchains
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Documents explicit Rust compiler and linker toolchain configuration. -->

# Rust Toolchains

Bessemer builds explicitly declared Rust targets with `rust_library`,
`rust_binary`, and `rust_test`. It schedules each compiler and linker action in
the build graph, so their declared inputs and outputs participate in Bessemer's
content-addressed cache.

```python
rust_library(
    name = "library",
    srcs = glob(["src/**/*.rs"]),
)

rust_binary(
    name = "app",
    srcs = ["src/main.rs"],
    deps = [":library"],
)

rust_test(
    name = "test",
    srcs = ["tests/test.rs"],
    deps = [":library"],
)
```

Build, run, and test those targets through the standard command interface:

```console
$ bsmr build //path/to/package:app
$ bsmr run //path/to/package:app -- --application-argument
$ bsmr test //path/to/package:test
```

These rules do not currently translate a Cargo workspace automatically. A
Cargo-first repository must generate or maintain equivalent Bessemer targets
before Bessemer can replace `cargo build` for that repository.

## Release Channels

The system Rust toolchain defaults to the stable release channel:

```python
load("@prelude//toolchains:rust.bzl", "system_rust_toolchain")

system_rust_toolchain(
    name = "rust",
    default_edition = "2024",
    visibility = ["PUBLIC"],
)
```

`system_rust_toolchain` uses `rustc`, `rustdoc`, and `clippy-driver` from the
action environment. It does not download or pin the latest stable toolchain and
is therefore intended for bootstrapping and local evaluation rather than
hermetic production builds.

Features that depend on unstable compiler behavior must declare the nightly
channel explicitly. Bessemer rejects nightly-only features under stable rather
than silently selecting another compiler:

```python
system_rust_toolchain(
    name = "rust",
    default_edition = "2024",
    nightly_features = True,
    release_channel = "nightly",
    visibility = ["PUBLIC"],
)
```

## Code Generation Backends

Nightly toolchains may provide a platform-specific rustc code generation
backend artifact. The artifact is an explicit compiler action input, so its
digest participates in the action key:

```python
system_rust_toolchain(
    name = "rust_cranelift",
    codegen_backend = select({
        "config//os:linux": "//toolchains/rust:cranelift_linux",
        "config//os:macos": "//toolchains/rust:cranelift_macos",
    }),
    default_edition = "2024",
    release_channel = "nightly",
    visibility = ["PUBLIC"],
)
```

The interface is backend-agnostic. It can host Cranelift, the GCC codegen
backend, or another rustc-compatible backend without adding backend-specific
logic to Rust rules. Compatibility, panic strategy, standard-library support,
and target coverage remain properties of the selected backend and compiler
version.

## Experimental Linkers

Rust linking uses the configured C++ linker toolchain. Alternative linkers can
therefore be evaluated without changing Rust rules. [Wild](https://github.com/davidlattimore/wild)
is a potential experimental Linux linker backend:

```python
load("@prelude//toolchains:cxx.bzl", "system_cxx_toolchain")

system_cxx_toolchain(
    name = "cxx",
    linker = "wild",
    visibility = ["PUBLIC"],
)
```

Wild is not a default or correctness-equivalent linker in Bessemer. In an
AArch64 Linux evaluation against mold 2.41.0 on Dedalus `dm-host-agent`, Wild
0.10.0 produced a smaller binary and lower median link latency, but Wild's own
`linker-diff` correctness comparison did not pass. Projects that opt in must pin
the Wild binary, run workload-specific correctness tests, and fail if it is
unavailable; Bessemer does not fall back to another linker.

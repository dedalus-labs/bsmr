---
id: cargo
title: Rust and Cargo
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Documents the native Cargo workspace API and its current hermeticity boundary. -->

# Rust and Cargo

The native Rust frontend is unreleased. These examples do not apply to the
earlier Cargo adapter in version 0.0.5.

Build a Cargo package without writing or synchronizing build rules:

```console
bsmr init
bsmr build app --show-output
bsmr test app
```

Run `bsmr build --help` or `bsmr test --help` for command options.

Commit `Cargo.toml`, `Cargo.lock`, and an exact `rust-toolchain.toml` at the
project root. The current catalog supports `1.97.1` and `nightly-2026-04-11` on
Linux and macOS, for ARM64 and x86-64. Install the matching Cargo resolver with
`rustup toolchain install <channel> --profile minimal` before building.

```toml title="rust-toolchain.toml"
[toolchain]
channel = "1.97.1"
profile = "minimal"
```

BSMR reads tracked manifests and file names into an isolated snapshot. The
selected Cargo resolves that snapshot with `metadata --frozen`, without registry
access or user Cargo configuration. Manifest and target-file changes invalidate
inference automatically. BSMR keeps the resulting graph private. It writes no
`BUILD.bsmr` files or ownership index into the checkout.

```text
Cargo files -> frozen resolution -> private targets -> native rustc actions
                                                 -> custom recipe actions
```

Each crate uses the existing native Rust compilation, linking, and test machinery.
A package containing one library or binary can be selected by its directory,
even when its Cargo name differs. Libraries also expose `:lib`, binaries expose
their Cargo target name, and dependency
renames preserve the name used in source. Inline unit tests are associated with
their build targets. Changing an unrelated crate does not recompile the selected
binary. Changing a dependency invalidates its consumers.

## Add another build step

A [custom recipe](../../recipes.md) can consume an inferred Rust executable
without restating its crate dependencies. Keep the recipe in its own package
so the Rust package retains its inferred definition.

## Supported boundary

The preview supports local path libraries, binaries, and their inline unit
tests. Features, build scripts, procedural macros, external sources, native
`links`, conditional dependencies, build/dev dependencies, integration tests,
and custom Cargo profiles or lints fail before compilation. Cross compilation
and Cargo command-line parity are not implemented. Shared files outside a crate
require explicitly declared action inputs.

Native actions validate rustc's reported source and environment reads before
accepting a successful compiler result. Undeclared reported inputs fail instead of
publishing an incomplete cache entry. This check is not a filesystem sandbox.

Rust compiler, Clippy, and standard-library archives have pinned SHA-256 digests.
Their content contributes to compilation action identity. Cargo metadata uses
the selected local rustup installation. C/C++ linking and Python bootstrap tools
still come from the execution host. This is not a fully hermetic or remotely
qualified toolchain. Native actions honor the existing cache-upload policy.

Ambient Rust flags and user Cargo configuration do not configure native actions.
Project `.cargo/config` files are rejected until their semantics are modeled.
Unsupported projects can use Cargo directly. BSMR never silently switches to a
whole-workspace Cargo build after inference or compilation fails.

[RFC 0002](https://github.com/dedalus-labs/bsmr/discussions/14) specifies the
remaining configured-unit graph, dependency acquisition, and build-script work.

## Verification

`test/native-rust-build.ts` exercises real compiler actions, unit-test success and
failure, and a custom recipe consuming an inferred executable. It checks
manifest invalidation, source invalidation, unrelated edits, cache restoration
in a second checkout, environment isolation, and build-script rejection. It also
checks that inference leaves the checkout free of generated build files.

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

BSMR builds conventional Cargo workspaces directly from their native ecosystem
files. You do not need a `BUILD.bsmr` or handwritten Starlark file.
BSMR lowers Cargo metadata into its private action graph; `Cargo.toml` and
`Cargo.lock` remain authoritative.

## Workspace contract

Commit these files at the repository root:

- `Cargo.toml` defining a package or workspace;
- `Cargo.lock`; and
- exactly one `rust-toolchain.toml` or `rust-toolchain` file.

The rustup channel must identify immutable compiler bits. BSMR accepts exact
stable versions such as `1.94.1` and dated nightlies such as
`nightly-2026-04-11`. Mutable aliases such as `stable` and `nightly` fail before
execution.

Initialize once, then address a Cargo package by its directory:

```console
bsmr init
bsmr targets packages/rust/dfa
bsmr build packages/rust/dfa
```

The package path resolves to one conventional target named after the final path
component. A virtual workspace root exposes `workspace`. Explicit BSMR labels
remain available for queries and automation.

## Execution and caching

The current adapter executes `cargo build --locked --manifest-path ...` with an
exact `RUSTUP_TOOLCHAIN`, isolated `CARGO_HOME` and `CARGO_TARGET_DIR`, disabled
Cargo incrementality, and deterministic source-path remapping. BSMR records the
complete declared output in its content-addressed store. A warm build performs
no Cargo action, and BSMR can restore a deleted output from the local CAS.

The first implementation deliberately runs locally and disables remote cache
upload. Cargo is still resolved from the host, and a cold action may fetch
locked crates from the registry. Remote-cache eligibility requires a
content-addressed Rust toolchain and a separate locked dependency-fetch action;
until those land, this is a cached native Cargo adapter rather than a fully
remote-hermetic Rust toolchain.

## Custom rules

An explicit Starlark build file takes precedence when a package needs a
non-conventional target. Choosing that file makes its rule graph authoritative;
BSMR does not silently alternate between native and custom implementations.

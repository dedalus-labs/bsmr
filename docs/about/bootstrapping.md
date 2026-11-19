---
id: bootstrapping
title: Bootstrapping Bessemer
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->


# Bootstrapping Bessemer

Bessemer's first binary is built with Cargo. Once compiled, its manifest
resolver can load the repository's `BUILD.bsmr` files.

For dependencies on Rust crates from [crates.io](https://crates.io), we use
[reindeer](https://github.com/facebookincubator/reindeer) to automatically
generate `BUILD.bsmr` files.

First, build `bsmr` and its Cargo planner with Cargo:

```sh
cargo build --locked --bin bsmr
cargo +1.98.0 build --locked --manifest-path tools/cargo/Cargo.toml --target-dir target
```

Both binaries land in `target/debug`. `bsmr` runs `bsmr-cargo` from its own
directory for Rust builds. The planner is a separate workspace pinned to Rust
1.98.0, which isolates Cargo's native dependencies from the engine.

Next, install [DotSlash](https://dotslash-cli.com) with Cargo:

```sh
cargo install --locked dotslash
```

Use `reindeer` to generate build manifests for Rust dependencies:

```sh
cd bsmr/
node ci/rust-build-dependencies.mjs
```

Verify that Bessemer can load the generated dependency graph:

```sh
target/debug/bsmr targets 'bsmr_build//...'
```

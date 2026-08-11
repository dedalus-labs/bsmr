---
description: Build Bessemer from source and run a first native Go build.
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines the source-build and native-Go quick start. -->

# Getting started

!!! warning "Pre-release software"

    Bessemer has not published a stable release. Build it from source and pin
    the commit used by your repository until the release contract is complete.

## Build Bessemer

The repository pins its Rust toolchain. Build the CLI from a clean checkout:

```shell
git clone https://github.com/dedalus-labs/bsmr.git
cd bsmr
cargo build --locked --bin bsmr
target/debug/bsmr --version
```

Keep the resulting executable at a stable path or place it on `PATH` for the
following commands.

## Initialize a Go repository

From a repository containing `go.mod` or `go.work`:

```shell
bsmr init --git
go mod vendor
bsmr go toolchain
bsmr go sync
bsmr build //cmd/server:bin
```

`bsmr go toolchain` selects the latest stable Go release for a new repository,
records the exact official archive identities, and installs the verified SDK
locally. An existing repository continues using its committed lock. Pass
`--update` to move that lock to the latest stable release or `--version 1.26.5`
to select an exact stable release.

`bsmr go sync` asks that SDK for structured package metadata and writes owned
build manifests. It does not reinterpret Minimal Version Selection and does not
download missing modules. The current frontend requires a checked-in vendor
tree for third-party packages.

## Commit the build identity

Commit the native Go metadata and generated IR:

- `.bsmr-go-toolchain.json`;
- `toolchains/bsmr_go_toolchain.bzl` and the updated `toolchains/BUCK`;
- `.bsmr-go-manifests`;
- generated package build files; and
- `go.mod`, `go.sum`, `vendor/modules.txt`, and the vendor tree.

Do not commit `toolchains/.bsmr-go-sdk` or `toolchains/.bsmr-go-tools`. They are
verified local materializations of the committed toolchain identity.

## Gate drift in CI

Run the check modes before building:

```shell
bsmr go toolchain --check
bsmr go sync --check
bsmr build //cmd/server:bin
```

Both checks are offline and read-only. They fail if the lock, acquired SDK,
generated toolchain, selected package graph, or owned manifests have drifted.

Continue with the [native Go guide](languages/go.md) for tags, tests, cgo, and
the current support boundary.

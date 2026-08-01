---
id: install
title: Installing Bessemer
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->


Bessemer has not published a stable binary release. Build the pinned source
toolchain instead:

```sh
git clone https://github.com/dedalus-labs/bsmr.git
cd bsmr
rustup show
cargo build --locked --release --bin bsmr
```

The binary is written to `target/release/bsmr`.

For development builds:

```sh
cargo build --locked --bin bsmr
target/debug/bsmr --help
```

The repository's [`rust-toolchain.toml`](../../rust-toolchain.toml) selects the
required compiler and components. On platforms without the vendored Protocol
Buffers compiler, set `BSMR_BUILD_PROTOC` and
`BSMR_BUILD_PROTOC_INCLUDE` before building.

<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# Installation

Bessemer has not published a stable binary release. Build the pinned source
toolchain:

```console
git clone https://github.com/dedalus-labs/bsmr.git
cd bsmr
cargo build --locked --release --bin bsmr
install target/release/bsmr ~/.local/bin/bsmr
bsmr --version
```

The repository's `rust-toolchain` file selects the compiler and components.
`cargo` installs that toolchain through rustup.

For Bessemer development, keep the debug binary in the repository:

```console
cargo build --locked --bin bsmr
target/debug/bsmr -h
```

Continue with the [Quick Start](quickstart.md).

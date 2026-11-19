<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# Installation

Bessemer publishes pre-1.0 binaries for macOS, Linux, and Windows. Releases in
the `0.0.x` series are versioned previews and may change their API between
versions.

On macOS or Linux:

```sh
curl --proto '=https' --tlsv1.2 --fail --location \
  https://github.com/dedalus-labs/bsmr/releases/latest/download/bsmr-installer.sh \
  --output bsmr-installer.sh
sh bsmr-installer.sh
```

On Windows, download `bsmr-installer.ps1` from the
[latest release](https://github.com/dedalus-labs/bsmr/releases/latest) and run
it in PowerShell. Each release includes platform archives, SHA-256 checksums,
and build provenance attestations.

To build from source instead, build the engine and the Cargo planner:

```console
git clone https://github.com/dedalus-labs/bsmr.git
cd bsmr
cargo build --locked --release --bin bsmr
rustup toolchain install 1.98.0 --profile minimal
cargo +1.98.0 build --locked --release --manifest-path tools/cargo/Cargo.toml --target-dir target
install target/release/bsmr target/release/bsmr-cargo ~/.local/bin/
bsmr --version
```

Install both binaries in one directory. `bsmr` runs `bsmr-cargo` from its own
directory to plan Rust builds, and Rust builds fail without it. Release
installers include both binaries.

The repository's `rust-toolchain` file selects the engine's compiler, and
`cargo` installs it through rustup. The planner is a separate Cargo workspace
pinned to Rust 1.98.0 in `tools/cargo/rust-toolchain.toml`.

For Bessemer development, keep both debug binaries in `target/debug`:

```console
cargo build --locked --bin bsmr
cargo +1.98.0 build --locked --manifest-path tools/cargo/Cargo.toml --target-dir target
target/debug/bsmr -h
```

Continue with the [Quick Start](quickstart.md).

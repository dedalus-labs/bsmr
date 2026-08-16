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

To build the pinned source toolchain instead:

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

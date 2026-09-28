<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# Bessemer

Bessemer (`bsmr`) builds packages from your project files and reuses unchanged
work. Supported native packages do not need handwritten build rules.

Bessemer is a preview. Releases in the `0.0.x` series may change their API.
TypeScript with pnpm is the primary integration. See [language support](docs/about/language_support.md)
for Rust, Go, and Python preview requirements.

## Install

On macOS or Linux:

```sh
curl --proto '=https' --tlsv1.2 --fail --location \
  https://github.com/dedalus-labs/bsmr/releases/latest/download/bsmr-installer.sh \
  --output bsmr-installer.sh
sh bsmr-installer.sh
```

Windows users can download and run `bsmr-installer.ps1` from the
[latest GitHub release](https://github.com/dedalus-labs/bsmr/releases/latest).
Every release also includes platform archives, SHA-256 checksums, and build
provenance attestations.

## Build a package

At your repository root:

```console
bsmr init
bsmr build apps/api --show-output
```

Replace `apps/api` with your package's directory. BSMR creates the project marker,
builds the package and its dependencies, and prints the output path. Run the same
build command after an edit to reuse unchanged work.

The [quick start](docs/getting_started/quickstart.md) covers setup.
The [TypeScript guide](docs/users/languages/typescript/pnpm.md) lists the required
pnpm and compiler files. The [Rust preview](docs/users/languages/rust/cargo.md)
uses Cargo manifests and an exact toolchain pin. [Custom recipes](docs/users/recipes.md)
connect additional steps through their inputs and outputs.

## Find a command

```console
bsmr --help
bsmr build --help
```

Use `bsmr -h` for the shortest command list. Each command's `--help` explains its
options. Run `bsmr clean` when you need to remove generated files and local state.

## Documentation

Read the [quick start](https://oss.dedaluslabs.ai/bsmr/getting_started/quickstart/)
or the [full documentation](https://oss.dedaluslabs.ai/bsmr/).

Build the documentation locally:

```console
python -m pip install -r docs/requirements.txt
python -m mkdocs serve -f mkdocs.yml
```

## Development

```console
cargo build --locked --bin bsmr
python3 test.py --ci --git --bsmr=target/debug/bsmr
pnpm install --frozen-lockfile --ignore-scripts
pnpm run ci check
```

## Provenance and license

Bessemer began as an upstream fork and now has its own product interface, native
ecosystem adapters, cache policy, release process, and roadmap. See
[`NOTICE`](NOTICE) for upstream provenance.

Except where an inherited notice states otherwise, Bessemer is licensed under
the [Apache License 2.0](LICENSE).

<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Introduces Bessemer's interface, supported ecosystems, and development workflow. -->

# Bessemer

Bessemer (`bsmr`) builds native TypeScript and Rust projects with a shared,
content-addressed cache. It reads `package.json`, `pnpm-lock.yaml`,
`Cargo.toml`, and `Cargo.lock`. Conventional projects do not need build files
or Starlark.

> [!NOTE]
> Bessemer is a preview. TypeScript with pnpm is the primary integration. Rust
> with Cargo is experimental. A stable binary release is not available yet.

## Interface

The beginner interface has three commands:

```console
bsmr init                 # Create .bsmr in the current project.
bsmr build <path>         # Build one native package and its dependencies.
bsmr clean                # Delete generated files and local build state.
```

Use package paths in normal work:

```console
bsmr build apps/api
bsmr build packages/rust/dfa
```

`bsmr -h` shows this small surface. `bsmr --help` shows the complete command
line, including target labels, graph queries, custom rules, and operational
tools.

## TypeScript and pnpm

A buildable package needs `package.json`, `tsconfig.json`, and
`tsdown.config.ts`. The workspace root needs `pnpm-workspace.yaml` and a frozen
`pnpm-lock.yaml`.

```console
bsmr init
bsmr build apps/api
```

Bessemer installs the exact pinned pnpm workspace once. It then schedules and
caches package builds independently.

## Rust and Cargo

A workspace needs `Cargo.toml`, `Cargo.lock`, and an exact Rust toolchain. Use
an exact stable version such as `1.94.1` or a dated nightly such as
`nightly-2026-04-11`.

```console
bsmr init
bsmr build packages/rust/dfa
```

Bessemer caches the complete Cargo output and can restore deleted outputs
without running Cargo again. The Cargo adapter is local-only until Rust
toolchains and registry inputs are fully content addressed.

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

Bessemer began as a Buck2 fork and now has its own product interface, native
ecosystem adapters, cache policy, release process, and roadmap. See
[`NOTICE`](NOTICE) and [`UPSTREAM_CHANGELOG.md`](UPSTREAM_CHANGELOG.md) for
upstream provenance.

Except where an inherited notice states otherwise, Bessemer is licensed under
the [Apache License 2.0](LICENSE).

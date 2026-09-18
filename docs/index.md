---
description: Fast, cached builds from native project files.
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# Bessemer

Bessemer (`bsmr`) builds TypeScript, Rust, Go and Python projects from native files.
You keep the ecosystem manifests and lock files your project already uses.
Bessemer creates the build graph, schedules work, and restores cached outputs.

```console
bsmr init
bsmr build apps/api
```

Conventional projects do not need build files or Starlark.

## What it does

| Need | Bessemer behavior |
| --- | --- |
| Build identity | Hashes declared inputs, tools, configuration, and dependency edges. |
| Fast rebuilds | Skips unchanged actions and restores missing outputs from a content-addressed store. |
| Native setup | Reads pnpm, Cargo, Go and Python project files. |
| Large repositories | Builds an explicit graph and schedules independent actions concurrently. |
| Custom behavior | Keeps labels, Starlark rules, queries, and remote execution in the advanced interface. |

## Support today

- **TypeScript and pnpm:** primary integration. Native package builds and
  typechecking are available.
- **Rust and Cargo:** experimental integration. Native package builds and local
  output caching are available.
- **Go:** experimental integration. Package synchronization, builds and tests
  use a verified SDK and vendored dependencies.
- **Python:** experimental integration. Wheels, lint, typechecking, tests and
  console scripts use `pyproject.toml`, PEP 751 locks and pinned tools.

BSMR is a preview. [Language support](about/language_support.md) states each
frontend's cache and isolation limits. [Capabilities](roadmap.md) covers the
execution profiles and verification commands.

## Start here

- [Quick Start](getting_started/quickstart.md)
- [TypeScript and pnpm](users/languages/typescript/pnpm.md)
- [Rust and Cargo](users/languages/rust/cargo.md)
- [Go](users/languages/go/native.md)
- [Command-line reference](reference/cli.md)
- [Configuration](reference/configuration.md)

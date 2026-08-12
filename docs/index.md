---
description: Fast, cached builds from native project files.
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# Bessemer

Bessemer (`bsmr`) builds TypeScript, Rust, and Go projects from their native files.
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
| Correct builds | Hashes declared inputs, tools, configuration, and dependency edges. |
| Fast rebuilds | Skips unchanged actions and restores missing outputs from a content-addressed store. |
| Native setup | Reads pnpm, Cargo, and Go project files directly. |
| Large repositories | Builds an explicit graph and schedules independent actions concurrently. |
| Custom behavior | Keeps labels, Starlark rules, queries, and remote execution in the advanced interface. |

## Support today

- **TypeScript and pnpm:** primary integration. Native package builds and
  typechecking are available.
- **Rust and Cargo:** experimental integration. Native package builds and local
  output caching are available.
- **Go:** experimental integration. Native package synchronization and
  hermetic pure-Go builds are available.
- **Python:** planned after Go.

BSMR is a preview. Read each language page before treating an action as fully
hermetic or remote-cache eligible.

## Start here

- [Quick Start](getting_started/quickstart.md)
- [TypeScript and pnpm](users/languages/typescript/pnpm.md)
- [Rust and Cargo](users/languages/rust/cargo.md)
- [Go](users/languages/go/native.md)
- [Command-line reference](reference/cli.md)
- [Configuration](reference/configuration.md)

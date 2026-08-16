<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# Quick Start

## 1. Initialize the workspace

Run this once at the repository root:

```console
bsmr init
```

This creates `.bsmr`. The file marks the project root and stores advanced
configuration. Most projects do not need to edit it.

## 2. Build one package

=== "TypeScript"

    ```console
    bsmr build apps/api
    ```

    Bessemer reads the pnpm workspace and frozen lockfile. It builds the selected
    package and the workspace packages it depends on.

=== "Rust"

    ```console
    bsmr build packages/rust/dfa
    ```

    Bessemer reads the Cargo workspace, lockfile, and exact Rust toolchain. It
    builds the selected package with `cargo build --locked`.

## 3. Build again

Run the same command. Unchanged actions use cached results. If a declared output
was deleted, Bessemer restores it from the local content-addressed store.

## Learn more only when needed

- Run `bsmr -h` for the beginner interface.
- Run `bsmr --help` for every command and global option.
- Read [TypeScript and pnpm](../users/languages/typescript/pnpm.md) for the exact
  workspace contract.
- Read [Rust and Cargo](../users/languages/rust/cargo.md) for the current local
  hermeticity boundary.
- Read [Command-line reference](../reference/cli.md) before using labels, graph
  queries, or custom rules.

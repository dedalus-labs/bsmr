<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# Build your first package

[Install Bessemer](install.md), then run these commands at your repository root:

```console
bsmr init
bsmr build apps/api --show-output
```

Replace `apps/api` with the directory of the package you want to build.
`bsmr init` creates `.bsmr` to mark the project root. The build command includes
the package's dependencies and prints the output path.

## Choose your project

For **TypeScript**, the package needs `package.json`, `tsconfig.json`, and
`tsdown.config.ts`. The workspace root needs its package manifest, pnpm workspace
file, and committed lockfile. The [TypeScript guide](../users/languages/typescript/pnpm.md)
explains the required tool versions and configuration.

**Rust, Go, and Python** have separate preview requirements. Check
[language support](../about/language_support.md) before using this workflow.
The [Rust preview](../users/languages/rust/cargo.md) infers packages directly.
[Go](../users/languages/go/native.md) currently requires a synchronization step.

## Build again

Run the same build command after an edit. BSMR reuses work whose inputs have
not changed. You do not need to clean the project between builds.

## Find an option

```console
bsmr --help
bsmr build --help
```

The first command lists commands and global options. The second explains build
options, including `--show-output`. Use `bsmr -h` for a shorter command list.
The [command-line reference](../reference/cli.md) contains the same parser's
commands and defaults.

When your build needs an extra step, add a [custom recipe](../users/recipes.md).
For cache behavior or advanced setup, see [caching](../concepts/caching.md) and
[configuration](../reference/configuration.md).

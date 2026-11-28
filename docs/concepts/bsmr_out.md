---
id: bsmr_out
title: bsmr-out
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->


# bsmr-out

Bessemer uses `bsmr-out` as the logical output prefix for build artifacts, backed
physically by `~/.bsmr/checkouts/<project-key>/<isolation>` in the user state
root. Bessemer does not create a `bsmr-out` directory inside your
[project](glossary.md#project) tree. You should not make assumptions about where
Bessemer places build artifacts within that directory structure as internal
locations can change over time. Instead, to obtain the location of the build
artifact for a particular target, use one of the `--show-full-*-output` options
with [`bsmr build`](../../users/commands/build) or
[`bsmr targets`](../../users/commands/targets). For the full list of ways to
show the output location, run `bsmr build --help` or `bsmr targets --help`.

```sh
bsmr targets --show-full-output <target>
bsmr build --show-full-output <target>
```

To locate the selected isolation's output directory without starting a daemon,
run `bsmr root --kind output`. Add `--dir PATH` to select another project, or
use `bsmr --isolation-dir NAME root --kind output` for a different isolation.
The command prints an absolute path under `~/.bsmr/checkouts` even when the
directory does not exist. Continue to use `build --show-full-json-output` for
individual artifacts.

## Upgrade from an in-tree `bsmr-out`

Bessemer does not read or migrate a former in-tree `bsmr-out` directory. Stop
older Bessemer daemons and delete any leftover `bsmr-out` directory from the
checkout. Repositories do not need `/bsmr-out` in `.gitignore`.

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

Bessemer stores build artifacts in a directory named `bsmr-out` in the root of your
[project](glossary.md#project). You should not make assumptions about where
Bessemer places your build artifacts within the directory structure beneath
`bsmr-out` as these locations depend on Bessemer's implementation and could
potentially change over time. Instead, to obtain the location of the build
artifact for a particular target, you can use one of the `--show-*-output`
options with the [`bsmr build`](../../users/commands/build) or
[`bsmr targets`](../../users/commands/targets) commands, most commonly
`--show-output`. For the full list of ways to show the output location, you can
run `bsmr build --help` or `bsmr targets --help`.

```sh
bsmr targets --show-output <target>
bsmr build --show-output <target>
```

To locate the selected isolation's output directory without starting a daemon,
run `bsmr root --kind output`. Add `--dir PATH` to select another project, or
use `bsmr --isolation-dir NAME root --kind output` for a different isolation.
The command prints an absolute path even when the directory does not exist.
Continue to use `build --show-full-json-output` for individual artifacts.

## Upgrade from the old output root

Bessemer does not read or migrate the former `bsmr-out` directory. Stop older
Bessemer daemons, delete that generated directory, and add `/bsmr-out` to the
repository `.gitignore`. New repositories created by `bsmr init --git` already
ignore the current directory.

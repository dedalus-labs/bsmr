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

## Worktree-aware output views

Set `BSMR_CHECKOUT_VIEW_DIR` to an absolute machine-level directory to keep
mutable output state outside each checkout. Bessemer assigns every canonical
project root its own view and exposes that view through the checkout's
`bsmr-out` symlink. It refuses an existing directory or a symlink whose target
does not match the assigned view.

At daemon startup, Bessemer removes inactive views whose checkout disappeared,
views older than 30 days, and then the oldest inactive views above the inactive
byte budget. Active daemon leases prevent collection. The default budget is ten
percent of the backing disk, rounded down to 5 GiB and clamped between 10 and
100 GiB. Override it with exact unsigned integer values in
`BSMR_CHECKOUT_VIEW_MAX_BYTES` and `BSMR_CHECKOUT_VIEW_MAX_AGE_SECS`.

This policy bounds mutable checkout state. The machine-wide action cache is
independent, so deleting a view does not delete reusable build results.

## Upgrade from the old output root

Bessemer does not read or migrate the former `bsmr-out` directory. Stop older
Bessemer daemons, delete that generated directory, and add `/bsmr-out` to the
repository `.gitignore`. New repositories created by `bsmr init --git` already
ignore the current directory.

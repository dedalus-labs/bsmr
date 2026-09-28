---
description: Fast, cached builds from native project files.
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# Bessemer

Bessemer (`bsmr`) builds packages from your project files and reuses unchanged work.
Start with one package in your repository:

```console
bsmr init
bsmr build apps/api --show-output
```

Replace `apps/api` with your package's directory. BSMR builds its dependencies
and prints the output path. Supported native packages do not need handwritten
build rules.

[Get started](getting_started/quickstart.md){ .md-button .md-button--primary }
[Install Bessemer](getting_started/install.md){ .md-button }

TypeScript with pnpm is the primary integration. Rust, Go, and Python are
previews with different requirements. See [language support](about/language_support.md)
to choose the guide for your project.

## Find what you need

Run `bsmr --help` to find commands and `bsmr build --help` for build options.
The [quick start](getting_started/quickstart.md) walks through your first build.
[Custom recipes](users/recipes.md) shows how to add another build step.

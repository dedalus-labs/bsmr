<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# Language support

BSMR distinguishes native support from inherited rules.

**Native support** means BSMR reads the ecosystem's normal manifests and lock
files. **Rule support** means the prelude can build the language after explicit
Starlark and toolchain configuration.

| Ecosystem | Native status | Default interface |
| --- | --- | --- |
| TypeScript and Node.js | Primary | pnpm workspace paths |
| Rust | Experimental | Cargo package paths |
| Go | Experimental | Native package synchronization |
| Python | Experimental | PEP 751 locks with pinned uv |

The inherited prelude also contains rules for C, C++, Java, Kotlin, Apple
platforms, Erlang, Haskell, OCaml, and other ecosystems. Those rules are
advanced extension points. Their presence does not mean BSMR offers a native,
zero-configuration workflow for that language.

Read the language-specific page for exact toolchain, cache, sandbox, and remote
execution boundaries.

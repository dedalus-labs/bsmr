<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# Language support

Choose the guide for your project. Each guide lists the supported configuration
and toolchain requirements.

| Language | Status | Setup |
| --- | --- | --- |
| [TypeScript](../users/languages/typescript/pnpm.md) | Primary | pnpm workspace and package directories |
| [Rust](../users/languages/rust/cargo.md) | Unreleased native preview | Cargo files and an exact Rust toolchain |
| [Go](../users/languages/go/native.md) | Experimental | Native package synchronization |
| Python | Experimental | PEP 751 lockfiles with pinned uv |

For native builds, BSMR reads the language's ordinary manifests and lockfiles.
The [Python guide is under review](https://github.com/dedalus-labs/bsmr/pull/96).

The inherited rules also support C, C++, Java, Kotlin, Apple platforms, Erlang,
Haskell, OCaml, and other ecosystems. These need explicit Starlark and toolchain
configuration. An available rule does not imply a native setup for that language.

See each guide for cache, sandbox, and remote execution boundaries.

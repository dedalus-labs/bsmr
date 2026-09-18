<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# Language support

BSMR reads native project files for TypeScript, Rust, Go and Python. The
frontends share scheduling and storage, with the following build contracts.

| Ecosystem | Available behavior | Boundary |
| --- | --- | --- |
| [TypeScript and pnpm](../users/languages/typescript/pnpm.md) | Package builds and typechecking with pinned Node/pnpm and a frozen lockfile. | Primary integration. Cold installs access the registry. Install hooks are disabled. |
| [Rust and Cargo](../users/languages/rust/cargo.md) | Native package builds with a locked toolchain selection and local output caching. | Experimental. Cargo comes from the host. Cold builds can fetch crates. Remote-cache upload is disabled. |
| [Go](../users/languages/go/native.md) | Package synchronization, compilation and tests with a verified SDK and declared dependencies. | Experimental. Modules must be vendored. Host-dependent linking and cgo remain outside portable cache eligibility. |
| [Python](https://github.com/dedalus-labs/bsmr/blob/main/docs/users/languages/python/overview.md) | Wheels, lint, typechecking, tests and console scripts from `pyproject.toml`, PEP 751 locks and pinned tools. | Experimental. Build backends lack enforced network isolation. Native extensions use local C/C++ tools and are not uploaded to a remote cache. |

Ordinary local builds do not prevent undeclared filesystem reads.
[Sandboxed execution](../users/sandboxing.md) enforces a separate, restricted
contract on x86-64 Linux/KVM. It accepts only actions compatible with that
profile. Native frontend support does not imply sandbox compatibility.

The inherited prelude also contains rules for C, C++, Java, Kotlin, Apple
platforms, Erlang, Haskell, OCaml, and other ecosystems. Those rules are
advanced extension points. Their presence does not mean BSMR offers a native,
zero-configuration workflow for that language.

Use explicit Starlark rules and toolchain configuration for those extensions.

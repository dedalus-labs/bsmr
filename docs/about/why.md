---
id: why
title: Why Bessemer
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->


Bessemer exists to provide one coherent build graph for large,
multi-language repositories. Build targets declare their inputs and outputs;
the engine evaluates only the affected graph and can execute actions locally
or through the Remote Execution API.

The implementation is derived from
[upstream](https://github.com/facebook/buck2), including its Rust engine,
Starlark rule surface, DICE incremental computation, and prelude. Bessemer
keeps that foundation while owning its public CLI, crate namespace, release
process, and future compatibility contract.

That ownership matters for a build tool. Repositories must be able to pin one
version, reproduce a build, inspect the dependency graph, and upgrade without
depending on another project's moving release channel.

Bessemer is pre-1.0 software. Its `0.0.x` releases provide immutable,
reproducible installation points, while the source and tests in this repository
remain the compatibility contract.

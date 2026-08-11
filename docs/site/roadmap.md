---
description: Product priorities and explicit non-claims for Bessemer.
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Summarizes ecosystem priorities without presenting proposals as releases. -->

# Roadmap

Bessemer's first-class ecosystem order is TypeScript, Rust, Go, then Python.
The shared engine should make each frontend feel native while preserving one
graph, action identity, CAS, and provenance model across a polyglot repository.

## Ecosystem contract

Each frontend should:

- treat native manifests and lock files as the developer interface;
- delegate dependency semantics to the ecosystem's authoritative resolver;
- normalize the resolved graph into Bessemer's action model;
- pin or verify toolchains and acquired artifacts;
- fail on drift or missing inputs instead of selecting another implementation;
  and
- expose generated manifests only as owned intermediate representation.

## Dependency snapshots

A future cross-ecosystem dependency snapshot should record an immutable,
content-addressed resolved universe rather than introduce a new universal
package solver. Native resolvers remain authoritative: pnpm for Node.js, Cargo
for Rust, uv for Python, and Minimal Version Selection for Go.

Bessemer can add the layer those tools do not share: parentage, fast Merkle
diffs, signed promotion evidence, compatibility results tied to the snapshot
digest, CAS reachability, and atomic rollback to a retained snapshot. A version
range alone is not compatibility evidence; successful builds and tests under
the exact resolved universe are.

This is a proposed direction, not a released command or storage format.

## Execution boundary

Local sandboxing and remote execution are deliberately separate from the
current native-build work. Remote caching is supported; the stronger execution
features remain proposals until their security and portability contracts are
implemented and measured.

Accepted designs are tracked in the repository's
[RFC discussions](https://github.com/dedalus-labs/bsmr/discussions?discussions_q=RFC).

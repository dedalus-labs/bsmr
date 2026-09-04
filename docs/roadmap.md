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
graph, action identity, content-addressed store (CAS), and provenance model
across a polyglot repository.

## Ecosystem contract

Each frontend should:

- treat native manifests and lock files as the developer interface;
- delegate dependency semantics to the ecosystem's authoritative resolver;
- normalize the resolved graph into Bessemer's action model;
- pin or verify toolchains and acquired artifacts;
- fail on drift or missing inputs instead of selecting another implementation;
  and
- expose generated manifests only as owned intermediate representation.

## DependencySet

A future cross-ecosystem DependencySet should state the components, interfaces,
compatibility ranges, platforms, patches, and provenance rules Bessemer may
accept. A separate content-addressed lock records one exact resolution. Native
resolvers remain authoritative: pnpm for Node.js, Cargo for Rust, uv for Python,
and Minimal Version Selection for Go.

Bessemer can add the layer those tools do not share: parentage, fast Merkle
diffs, signed promotion evidence, compatibility results tied to the
DependencySet and lock digests, CAS reachability, and atomic rollback to a
retained lock. A version range alone is not compatibility evidence; successful
builds and tests under the exact resolution are.

This is a proposed direction, not a released command or storage format.

The exact lock, compatibility, patch, cache, and deployment contract is being
specified in
[RFC 0004](rfcs/0004-dependency-set.md). The public
[DependencySet overview](concepts/dependency_set.md) distinguishes rules, exact
resolution, and release certification.

## Execution boundary

Local sandboxing and remote execution are deliberately separate from the
current native-build work. Remote caching is supported; the stronger execution
features remain proposals until their security and portability contracts are
implemented and measured.

Accepted designs are tracked in the repository's
[RFC discussions](https://github.com/dedalus-labs/bsmr/discussions?discussions_q=RFC).

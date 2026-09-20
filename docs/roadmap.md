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

## Build from project files

The intended starting point is `bsmr init`, followed by `bsmr build <package>`.
Project files should supply the dependency and compiler configuration. Users
should not maintain a second copy of that information in build rules.

Custom recipes describe only additional work. They declare inputs and outputs
that compose with inferred packages in the same graph. Commands should teach
their usage through `--help`, with deeper contracts in the language guides.

## Ecosystem contract

Each frontend should:

- treat native manifests and lock files as the developer interface;
- delegate dependency semantics to the ecosystem's authoritative resolver;
- normalize the resolved graph into Bessemer's action model;
- pin or verify toolchains and acquired artifacts;
- fail on drift or missing inputs instead of selecting another implementation;
  and
- keep the generated graph private instead of requiring users to synchronize it.

## Rust qualification

The native Rust preview covers local path libraries, binaries, and inline unit
tests. Production Cargo compatibility remains open. The next milestone is one
representative application and its tests, built with its existing configuration.

| Milestone | Required evidence |
|---|---|
| Cargo compatibility | Preserve features, profiles, platform conditions, test dependencies, build scripts and procedural macros. Compare results with Cargo on the same project. |
| Dependency acquisition | Reproduce locked registry and Git dependencies on a fresh worker. Reject checksum and lockfile drift. |
| Local reuse | Measure cold, warm, leaf-edit and shared-dependency builds. Verify changed inputs miss and unchanged outputs match. |
| Shared-cache qualification | Restore on a second isolated worker with local execution. Test compiler, target, flags, environment and dependency changes. Bound storage and preserve active results during eviction. |
| Build-system comparison | Run equivalent build and test workloads against Bazel with matched toolchains, resources, cache states and correctness checks. Report raw samples and limitations. |

Matching or improving on Bazel is a goal, not a measured result. Faster target
discovery does not establish faster compilation or production readiness.
Remote execution is outside the current milestone. A shared cache can be
qualified while all compiler actions continue to execute locally.

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

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

## Rust builds

The native Rust preview covers hook-free local libraries, binaries and inline
unit tests. Production Cargo compatibility is the next gate. Qualify real
applications with their existing manifests, lockfiles and configuration.

### Build existing Cargo projects

Cargo remains the authority for dependency and feature resolution. BSMR must
preserve the resulting configured compiler actions before optimizing them.

- Model target-specific compiler flags, linkers, profiles, package overrides
  and host build overrides. Map storage-only Cargo settings to BSMR's storage
  policy without silently dropping compiler settings.
- Acquire locked registry, Git and patched sources with integrity checks.
- Resolve features per configured unit, including distinct host and target
  dependencies. Compile procedural macros for the host.
- Model build-script outputs, environment, rerun inputs and native-link metadata.
  Native libraries need declared C/C++ tools and inputs.
- Preserve conditional and development dependencies, integration tests,
  benchmarks and examples.
- Isolate build identity in a small final stamping action. A revision embedded
  in an artifact must remain an input to the action producing those bytes.

Qualify real applications with their existing dependencies and configuration.
Reject unsupported behavior explicitly until its semantics are implemented and
compared with Cargo.

### Make the edit loop fast

Reuse the engine's existing metadata-check actions, dependency graph, content
cache and local incremental machinery. The CLI already exposes artifact paths,
`--out` and JSON build reports. Extend the native frontend around those paths.

Measure metadata-only checks, dependency edits, worktree changes, test selection
and output restoration. Keep local mutable incremental state separate from
portable cached compiler results. Preserve useful short and structured errors,
test filters and explanations of executed work.

Unchanged public signatures alone do not prove that a dependent can be skipped.
Inline and generic bodies can affect Rust metadata. Unchanged metadata may
permit downstream checks to be reused, while changed object code still needs
relinking. Each claimed cutoff needs a compiled-output regression test.

A shared cache should reuse identical actions across worktrees and qualified
hosts, with bounded storage and safe eviction. Compare against Cargo with both
fresh build directories and a deliberately shared build directory. Cargo does
not inherently recompile every external dependency for every worktree.

Linux builds initiated from macOS need an explicit worker boundary and a
content-addressed source transfer. Keep that transport separate from scheduling
individual remote actions. Remote execution is outside the current milestone.

### Reuse expensive fixtures

Recipes should compose inferred binaries with non-Rust inputs, including kernel
sources, boot images and VM snapshots. Declare device and execution requirements
for actions that need hardware access. Store large outputs by content reference
and verify isolation when materializing writable copies.

### Verify build results and performance

Measure existing project build commands and BSMR with the same dependencies,
profiles, toolchains and resources. Include the project's configured caches.
Separate cold builds, warm builds, source edits, fresh checkouts and cache
restoration. Record executed work and verify artifacts or behavior before
comparing elapsed time.

Faster target discovery does not establish faster production compilation.
Unsupported targets have no successful-build timing. Publish README speed
claims only after representative, reproducible comparisons pass.

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

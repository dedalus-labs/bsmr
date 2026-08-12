---
rfc: "0003"
title: "Build Go packages hermetically from the selected module graph"
authors: ["@windsornguyen"]
state: discussion
discussion: "https://github.com/dedalus-labs/bsmr/discussions/15"
labels: ["go", "ecosystems", "hermeticity", "performance"]
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines Bessemer's native Go graph, toolchain, and execution boundary. -->

# Build Go packages hermetically from the selected module graph

## Summary

Bessemer will use the official Go command as the authority for module,
workspace, build-constraint, package, test, and embed semantics. It will import
that structured graph and execute compilation, assembly, packing, linking, and
tests as ordinary fine-grained Bessemer actions backed by DICE and the CAS.

The design keeps `go.mod`, `go.sum`, optional `go.work`, and Go tooling as the
developer interface. It does not introduce a second dependency lock or ask
developers to restate imports in Starlark. The full requirement set remains in
[Discussion 15](https://github.com/dedalus-labs/bsmr/discussions/15).

## Context

`go.sum` authenticates module content; it does not record the selected module
build list. The exact Go release computes that list from module and workspace
files using version-specific Minimal Version Selection semantics. Reimplementing
that algorithm inside Bessemer would create a second Go implementation without
improving build execution.

The Bessemer prelude already models Go compiler, assembler, packer, linker,
standard-library, cgo, and test actions. The missing layer is a native frontend
that converts Go's selected graph into those targets without a second
hand-maintained graph.

## Goals and non-goals

The design must provide:

- exact Go graph semantics from a selected SDK;
- offline frozen builds from declared repository, SDK, and CAS inputs;
- package-granular compilation, invalidation, remote caching, and provenance;
- stable generated targets for libraries, binaries, and tests;
- verified official SDK acquisition and verified module acquisition in the
  completed design; and
- actionable failure for unsupported graph, toolchain, cgo, or test semantics.

The design will not reimplement Minimal Version Selection, replace native Go
metadata, run module mutation or `go generate` implicitly, inspect an ambient
module cache during frozen builds, or silently fall back to `go build`.

## Determination

### Separate mutation, acquisition, and build

Dependency updates remain explicit native Go operations. SDK and module
acquisition may use the network only as an explicit operation that verifies and
stores immutable content. Frozen graph import and action execution may not
repair metadata, change selected versions, download a toolchain, or fetch
source.

Frozen Go invocations set `GOTOOLCHAIN=local`. Graph import ignores ambient
`go env -w` state and disables proxy and checksum-database access. A missing
input is an error naming the package or artifact.

### Import, do not reinterpret, the graph

The selected SDK emits structured `go list -deps -json -test` metadata for the
declared module/workspace, target, tags, cgo state, and test mode. Bessemer
validates and normalizes that graph, discards synthetic test variants, rejects
nodes outside its declared source boundary, and lowers direct imports to stable
target labels.

The graph uses Kahn's algorithm for deterministic topological ordering. Disjoint
sets are not relevant: imports form a directed acyclic graph, and dependency
order and reverse invalidation matter more than undirected connectivity.

### Execute ordinary Bessemer actions

Generated targets use the existing `go_library`, `go_binary`, and `go_test`
rules. Their compiler, standard-library, assembly, pack, link, embed, and test
actions therefore use the same execution platform, DICE invalidation, CAS,
materializer, remote-cache, and observability machinery as other Bessemer
targets.

Action identity must include selected source and embed files, direct dependency
archives, exact SDK and tools, target platform, build tags, cgo/native toolchain,
compiler/linker settings, and rule implementation. Cross-machine cache upload is
permitted only when those inputs are declared.

### Own generated files narrowly

The frontend generates one manifest beside each selected package and records
the paths in `.bsmr-go-manifests`. It writes only files bearing its exact marker,
removes only obsolete paths in that ownership index, and fails rather than
overwrite user-authored build files. `bsmr go sync --check` is the CI drift gate.

### Acquire exact toolchains and modules

`bsmr go toolchain` resolves the latest stable Go SDK for a new repository or
acquires the exact committed lock in an existing repository. `--update` moves an
existing lock to the latest stable release, while `--version` selects one exact
stable release such as `1.26` or `1.26.5`. The command records the official
archive name, SHA-256
digest, and byte length for Darwin and Linux on amd64 and arm64, verifies the
extracted SDK's `VERSION`, and installs a repository-local ignored SDK plus
bootstrap wrapper.
The installed tree is hard-linked from Bessemer's materialized archive, so the
immutable bytes are not duplicated. `bsmr go toolchain --check` performs the
offline lock, generated-IR, acquisition-metadata, and SDK-version drift gate.

Every repository resolves to an exact committed version. Configured mirrors may
eventually serve identical bytes but cannot change artifact identity.

Module acquisition will preserve module path, selected version, replacement,
zip and `go.mod` checksums, source, and privacy policy as provenance. Credentials
remain at the acquisition boundary and never enter action keys or logs.

## Current implementation

The first implementation slice provides `bsmr go sync` for one repository root:

- exact official SDK selection, verification, and repository-local acquisition;
- SDK-selected source, cgo, assembly, syso, test, and embed files;
- direct local and vendored import edges;
- stable `:lib`, `:bin`, `:test`, and `:external_test` target names;
- explicit build tags and cgo selection;
- deterministic manifests and ownership-index checking; and
- ordinary Bessemer Go compilation/link actions with local and REAPI caching.

This slice intentionally requires vendored third-party source. It uses a
single acquired SDK for both graph import and execution. Verified module
acquisition, cross-platform cgo conformance, and advanced test modes remain
release gates rather than implied support. Pure-Go targets use the existing
Darwin/Linux and amd64/arm64 target-platform machinery. The current host-native
cgo lane builds and tests package-local C implementations and checks the same
cold, incremental, restoration, and remote-cache regimes as pure Go against
Bazel/rules_go.

The frontend does not expose generated Starlark as a developer interface.
Native Go metadata is authoritative; generated build files are owned IR that
feeds the existing Bessemer graph and action machinery.

### Hermeticity boundary

Pure-Go graph import and builds use declared repository inputs, the exact
verified SDK, explicit action environments, deterministic keys, and no network.
This is a hermetic input and toolchain contract, but it is not yet an enforced
filesystem-isolation boundary: without sandboxing, an incorrectly authored
action could still read an undeclared host path. Bessemer does not claim that
stronger property until it can enforce it.

Remote action-cache and CAS restoration remain supported. Remote execution and
sandboxing are explicitly outside this implementation scope.

Host-native cgo additionally consumes the configured system C/C++ toolchain and
SDK. It is correct for the declared host lane but is not fully hermetic until
Bessemer can pin and verify that native toolchain and sysroot. Cross-cgo is not
supported.

## Alternatives

Keeping `go build` as an opaque permanent action preserves native behavior but
prevents package-granular execution, cache reuse, provenance, and
affected-target analysis. Reimplementing Go module selection increases
correctness risk without improving compilation. Adopting rules_go and Gazelle
directly provides excellent precedent and a conformance oracle, but their Bazel
repository and provider model is not Bessemer's graph or execution interface.

## Consequences

Developers retain native Go dependency workflows and gain stable Bessemer
targets, fine-grained invalidation, and shared cache restoration. The frontend
adds a sync step after package-graph changes and initially requires vendoring.

Security improves only within the declared boundary. Pure-Go cache sharing may
use the exact SDK, environment, rule, platform, dependency, and source identity.
Cgo cache sharing must remain scoped to a verified native-toolchain identity.

Performance has two distinct costs: graph synchronization invokes the Go
command, and fine-grained execution schedules more actions than one opaque
`go build`. Warm invalidation and distributed reuse should repay that overhead;
benchmarks must publish cases where they do not.

## Validation and rollout

The initial frontend requires unit fixtures for graph normalization, cycles,
unsafe paths, non-vendored imports, manifest determinism, ownership, configured
build files, tags, and cgo mode. End-to-end tests must build and run generated
libraries, binaries, and internal tests.

The reproducible Go benchmark compares equivalent pure-Go and cgo Bessemer and
Bazel/rules_go graphs across cold, no-op, private edit, exported API edit,
unrelated edit, and output-restoration regimes. It rejects output differences,
remote clones that differ from the populated cache, and mismatched logical
action cuts. Reports record every sample, tool version, machine, cache endpoint,
action count, and median.

The RFC may become `implemented` only after verified module acquisition,
offline replay on a clean machine, cross-platform conformance, the declared cgo
matrix, remote-cache restoration, differential fixtures against the supported
Go releases, and published benchmark methodology all pass. Exact SDK
acquisition and external tests are implemented prerequisites, not open gates.

### Kubernetes scale probe

A development probe at Kubernetes commit
`d244fad12002f3e85ed6d3ee9ad6664d154e5d04` imported 3,383 packages and built
`//cmd/kube-apiserver:bin` from an empty Bessemer action cache with the ambient
`PATH` removed and all network proxies pointed at a closed local port. The build
completed 4,160 local commands. The immediate no-op analyzed zero targets, ran
zero actions, materialized zero files, and completed in 0.3 seconds.

A private leaf edit selected exactly four actions: three package compilations
and the final link. Its wall time is intentionally not reported because the
validation host was 98% full and actively swapping; the run proves the
invalidation cut, not representative performance. Publishable scale benchmarks
must run through the reproducible harness on an uncontended host.

## Open questions

- The initial official Go release/platform support window needs a concrete
  compatibility policy. The default should track latest stable while each build
  resolves to an exact verified SDK.
- Vendor mode is implemented first because it gives frozen builds a simple
  source boundary. Direct verified module-CAS acquisition should replace the
  repository-size cost without changing graph semantics.
- Cross-platform sync may use one exact SDK with explicit `GOOS` and `GOARCH`,
  but conformance must prove its file selection matches execution toolchains.
- Public monorepos for scale and cgo conformance remain to be selected alongside
  representative Dedalus repositories.

## References

- [RFC discussion and complete requirements](https://github.com/dedalus-labs/bsmr/discussions/15)
- [Deferred native Go developer surfaces](https://github.com/dedalus-labs/bsmr/issues/58)
- [Go Modules Reference](https://go.dev/ref/mod)
- [Go toolchain selection](https://go.dev/doc/toolchain)
- [Go package listing](https://pkg.go.dev/cmd/go#hdr-List_packages_or_modules)
- [rules_go toolchains](https://github.com/bazel-contrib/rules_go/blob/master/docs/go/core/toolchains.rst)
- [Gazelle](https://github.com/bazel-contrib/bazel-gazelle)
- [Go benchmark contract](https://github.com/dedalus-labs/bsmr/blob/main/benchmarks/README.md#go-builds)

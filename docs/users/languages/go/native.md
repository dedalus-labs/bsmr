---
description: Native Go package discovery, targets, toolchains, and limitations.
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Documents the supported native Go frontend contract. -->

# Go

Bessemer treats the official Go SDK as the authority for modules, workspaces,
build constraints, source selection, tests, and embeds. It imports that graph
and lowers it into ordinary Bessemer targets backed by DICE and the CAS.

## Native package discovery

```shell
go mod vendor
bsmr go toolchain
bsmr go sync
bsmr build //cmd/server:bin
bsmr test //pkg/store:test
```

Synchronization invokes the locked SDK's `go list -deps -json -test` with
module writes, network access, ambient `go env -w` state, and automatic
toolchain switching disabled. A dependency must be in the repository, the
vendor tree, or the selected standard library.

Every package named by a `go.mod` `tool` directive is also a root. A tool
vendored as `sigs.k8s.io/controller-tools/cmd/controller-gen` becomes
`//vendor/sigs.k8s.io/controller-tools/cmd/controller-gen:bin`, which
`bsmr run` executes. Tool directives require Go 1.24 or later.

| Go package | Generated target |
| --- | --- |
| library | `//path/to/package:lib` |
| `package main` | `//path/to/package:bin` |
| internal tests | `//path/to/package:test` |
| external tests | `//path/to/package:external_test` |

The generated manifests record exact source and embed files, direct imports,
canonical package identity, selected tags, and cgo mode. They carry an ownership
marker, and synchronization refuses to overwrite a human-authored build file.
When a package leaves the graph, synchronization deletes its generated
manifest, and `bsmr go sync --check` reports that manifest as stale. This
includes the last package of a module, such as a tool whose `tool` directive
was removed.

Internal tests inherit the embedded files of their `target_under_test`. A path
declared by both the test and its target must refer to the same source artifact.
Run `node test/native-go-build.ts /path/to/bsmr` to verify a binary and internal
test that share an embedded file with Go 1.26.7. The same fixture checks cache
restoration, source-root reuse, input invalidation, and system-tool exclusion.
Use a binary built from the same revision as the prelude and native generator.

## Build info

A synchronized `go_binary` embeds the `debug.BuildInfo` that `go build
-trimpath` records for the same vendored graph: the package path, its module as
`mod`, each linked module as `dep` with its replacement, and the build mode,
compiler, tags, cgo mode, and target platform. A tool such as `controller-gen`
therefore reports its release version. `bsmr go sync` writes the module lines into
the generated `modules` attribute, so a dependency version change relinks the
binary.

Two settings are left out. VCS stamps (`vcs.revision`, `vcs.time`,
`vcs.modified`) depend on repository state that is not an action input, so
recording them would break cache reuse. `DefaultGODEBUG` is omitted because
the link does not yet apply the module's GODEBUG defaults.

## Build tags

Declare every selectable tag in `.bsmr` so graph selection and action
identity cannot diverge:

```ini
[go]
allowed_build_tags = integration,enterprise
```

Then select tags during synchronization:

```shell
bsmr go sync --tags integration
```

`bsmr go sync --check --tags integration` verifies the same configuration in
CI. An undeclared tag is an error.

## Toolchain identity

The committed toolchain lock records the exact Go version plus the official
archive name, SHA-256 digest, and byte length for Darwin and Linux on amd64 and
arm64. The execution host selects runnable SDK tools independently of the
target platform.

Pure-Go builds use the existing Darwin/Linux and amd64/arm64 target-platform
machinery. An arm64 Darwin runner can therefore execute arm64 Darwin tools that
emit a Linux amd64 pure-Go binary without confusing execution identity with
target identity.

## Reuse compiled work

A fresh checkout can reuse compiled packages when its declared source, SDK,
helper tools, and target match a cached action. The generated native toolchain
enables this cache for artifact-backed Go tools. Generic toolchains remain
ineligible unless their rule author explicitly declares those inputs.

Keep the shared cache outside disposable workspaces:

```shell
export BSMR_LOCAL_CACHE_DIR=/absolute/persistent/bsmr-cache
bsmr build //cmd/server:bin -c go.link_mode=internal
```

The setting supplies a link mode only when a rule omits `link_mode`. Selecting
`internal` makes Go's SDK linker the implementation and includes that choice
in the action key. Cache eligibility never changes the selected mode.

Automatic or external links can invoke a host C linker and remain uncached.
Cgo links, additional rule-level linker flags, system Go, and the default
system-Python bootstrap also remain outside this cache eligibility. Cached
outputs are restored as writable copies, independently of their stored bytes.

The fixture deletes worktree outputs and restarts the daemon before checking
restoration. It also checks another source root, a policy-only file change,
compiled-source and SDK-input edits, and repeated system-tool builds. These
results establish local action-cache behavior, not remote execution or local
filesystem isolation.

## cgo

Pass `--cgo` to include the files selected by host-native cgo:

```shell
bsmr go sync --cgo
bsmr build //cmd/server:bin
```

The current path supports package-local Go, C, C++, header, assembly, and
system-object inputs. Objective-C, Fortran, SWIG, and cross-cgo are rejected.

Host-native cgo consumes the configured system C/C++ toolchain and SDK. It is
correct for that declared host lane but is not fully hermetic until Bessemer can
pin and verify the native toolchain and sysroot.

## Current boundary

- Third-party modules must be checked into `vendor/`.
- Pure-Go actions have declared repository, SDK, platform, and dependency
  inputs and run without network access.
- Local actions are not yet filesystem-sandboxed.
- Remote action-cache and CAS restoration are supported.
- Remote execution is outside the current implementation scope.

The full design, consequences, benchmarks, and release gates live in
[RFC 0003](https://github.com/dedalus-labs/bsmr/blob/main/docs/rfcs/0003-native-go-builds.md).

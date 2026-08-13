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

| Go package | Generated target |
| --- | --- |
| library | `//path/to/package:lib` |
| `package main` | `//path/to/package:bin` |
| internal tests | `//path/to/package:test` |
| external tests | `//path/to/package:external_test` |

The generated manifests record exact source and embed files, direct imports,
canonical package identity, selected tags, and cgo mode. They carry an ownership
marker, and synchronization refuses to overwrite a human-authored build file.

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

---
id: overview
title: Overview
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->


# Overview

This is an overview of using Bessemer to build Go projects. It assumes you have a
basic understanding of Bessemer and Go. If you are completely new to Bessemer, see the
[Bessemer Getting Started](../../../getting_started/index.md) to learn the basic
concepts.

## Native package discovery

`bsmr go sync` converts the package graph selected by the official Go command
into ordinary Bessemer targets. Developers keep `go.mod`, optional `go.work`, Go
build constraints, and `go:embed`; they do not maintain a second dependency
graph by hand.

```shell
go mod vendor
bsmr go toolchain
bsmr go sync
bsmr build //cmd/server:bin
bsmr go sync --check
bsmr go toolchain --check
```

`bsmr go toolchain` selects the latest stable release when no lock exists and
otherwise acquires the committed release. It records the exact official archive
identities for Darwin and Linux on amd64 and arm64. Pass `--update` to move an
existing lock to the latest stable release, or `--version` with an exact release
such as `1.26` or `1.26.5`. The acquired SDK and bootstrap tools are verified,
repository-local ignored inputs; synchronization and builds do not depend on an
ambient `go` executable.

Synchronization invokes that SDK's `go list -deps -json -test` with module
writes, network-backed dependency resolution, ambient `go env -w` state, and
automatic toolchain switching disabled. It fails if a required dependency is
neither in the repository nor the standard library. Pass build tags with
`--tags`, and opt into host-native cgo file selection with `--cgo`. Tags must
also be declared in the root configuration so Bessemer can represent them as
configuration transitions:

```ini
[go]
allowed_build_tags = integration,enterprise
```

| Go package | Bessemer target |
| --- | --- |
| library package | `//path/to/package:lib` |
| `package main` | `//path/to/package:bin` |
| package with internal tests | `//path/to/package:test` |
| package with external tests | `//path/to/package:external_test` |

### What synchronization owns

Synchronization has one directional contract:

1. The locked SDK evaluates `go.mod`, optional `go.work`, vendored modules,
   build tags, cgo mode, and source constraints.
2. Bessemer validates the returned package records and normalizes direct imports
   into a deterministic, repository-local graph.
3. Bessemer renders that graph as owned build manifests. Those manifests are
   intermediate representation; developers continue to edit native Go files.
4. `bsmr go sync --check` reruns the same import and fails if committed output
   differs, without changing the repository.

The generated and acquired artifacts have deliberately different lifetimes:

| Artifact | Commit? | Why it exists |
| --- | --- | --- |
| `.bsmr-go-toolchain.json` | yes | Pins the SDK semantics and authenticated archives for every supported execution host. |
| `toolchains/bsmr_go_toolchain.bzl` | yes | Lowers the lock into execution-host archive and target-platform selections. |
| `toolchains/BUILD.bsmr` | yes | Activates the generated Go toolchains without replacing the other language toolchains. |
| `.bsmr-go-manifests` | yes | Records the exact manifest paths Bessemer may later remove as stale. |
| `<package>/<buildfile>` | yes | Carries the validated native graph into ordinary Bessemer rules and action keys. |
| `toolchains/.bsmr-go-sdk` | no | Holds the verified SDK executable and standard library for the current host. |
| `toolchains/.bsmr-go-tools` | no | Holds the bootstrap wrapper compiled by that SDK for the current host. |

Generated manifests carry their source files, embed files, direct imports,
build tags, cgo mode, canonical import path, and a strict ownership marker.
Synchronization refuses to overwrite a human-owned build file. The
`.bsmr-go-manifests` index lets it remove only obsolete files that it previously
generated.

The rule attributes are correctness inputs rather than generated ceremony:

| Attribute | Purpose |
| --- | --- |
| `package_name` | Preserves Go's canonical import identity for compilation and external tests. |
| `srcs` | Records the exact files selected by the locked SDK for the active configuration. |
| `embed_srcs` | Makes every `go:embed` match an explicit action input. |
| `deps` | Gives each compile action only its direct local or vendored import archives. |
| `build_tags` | Keeps graph selection and compile action identity on the same configured tags. |
| `cgo_enabled` / `override_cgo_enabled` | Prevents pure-Go and host-native cgo graphs from sharing an incorrect action identity. |
| `target_under_test` | Reuses the production package for internal tests instead of recompiling an unrelated package shape. |
| `package_name = "<import>_test"` | Keeps external tests in the separate package that Go semantics require. |

The native cgo path supports package-local Go, C, C++, header, assembly, and
system-object inputs. Generated targets preserve Go's quoted-header lookup
semantics and enable the configured C/C++ toolchain for compilation and external
linking. Objective-C, Fortran, and SWIG inputs remain unsupported and fail
during synchronization.

The initial frontend supports packages inside one synchronization root,
including a checked-in vendor tree and external test packages (`package
foo_test`). Verified module acquisition and cross-platform cgo remain outside
the current supported surface and fail rather than silently selecting another
implementation. Pure-Go builds can use the existing Darwin/Linux and
amd64/arm64 target platforms.

## Toolchain configuration

Graph discovery and execution are separate boundaries, but both use the exact
SDK selected by `bsmr go toolchain`; see [Toolchains](toolchains.md). Native Go
metadata is the only developer interface. Generated Starlark manifests are
Bessemer-owned IR and should not be edited.

Pure-Go actions have exact declared source, dependency, SDK, environment, and
platform identities and run without network access. Bessemer does not yet
sandbox local actions or remotely execute them, so it does not claim to enforce
filesystem isolation against an incorrectly authored action. Remote action-cache
and CAS restoration are supported. Host-native cgo also depends on the system
C/C++ toolchain and is not fully hermetic until that toolchain and sysroot are
pinned.

The [Go toolchain example](https://github.com/dedalus-labs/bsmr/tree/main/examples/toolchains/go_toolchain)
shows the underlying configuration. A minimal workflow is:

```shell
bsmr go sync
bsmr build //cmd/server:bin
bsmr test //pkg/store:test
```

## The types of targets

- `go_binary` - a binary target (`package "main"`)
- `go_library` - a library target (other packages)
- `go_test` - a test target (tests for any packages)
- `go_exported_library` - a target that exports a C-compatible interface for Go
  code (a special case of `package "main"`)

## Handwritten Go targets

Bessemer also supports handwritten targets for cases that need attributes the
native frontend does not generate yet. Keep one Go package per directory and
make its dependencies explicit:

- Keep a single Go package per directory. For example, for a Go library, all
  non-test `.go` files should belong to a single `go_library` and all
  `*_test.go` files to a single `go_test`.
- Put a `BUILD.bsmr` file in the same directory as the Go package, unless you have a
  reason not to.

```python
# File: foo/BUILD.bsmr

go_library(
    name = "foo",
    srcs = glob(["*.go"], exclude = ["*_test.go"]),
    deps = [
        "//path/to/other:lib",
    ],
)

go_test(
    name = "foo_test",
    srcs = glob(["*_test.go"]),
    target_under_test = ":foo",
    deps = [
        "//path-to-third-party/vendor/go/github.com/stretchr/testify:assert",
    ],
)
```

## How to pass options to `bsmr` commands

### Envs GOOS and GOARCH

Compilation for different platforms is done by passing `--target-platforms` or
`-m` (`--modifier`) flags to `bsmr` commands.

You need to specify what target platforms you support by declaring them with the
`platform()` rule, or you can avoid pre-declaring them by using configuration
modifiers.

For example, to build for linux/amd64, the following commands are equivalent
(assuming your project confugured similary to
[this example](https://github.com/dedalus-labs/bsmr/tree/main/examples/toolchains/go_toolchain)):

```sh
$ GOOS=linux GOARCH=amd64 go build example.com/foo/bar
$ bsmr build --target-platforms root//platforms:linux_x86_64 root//foo/bar:bar
$ bsmr build -m config//os:linux -m config//arch:x86_64 root//foo/bar:bar
```

### Test options like `-test.bench`

To pass test options, use `--` to separate bsmr options from test options:

<OssOnly>
```sh
$ bsmr test root//foo/bar:bar -- -test.bench=.
```
</OssOnly>
<FbInternalOnly>
> **Note:** You need to use `run` instead of `test` otherwise you'll be passing options to TPX
```sh
$ bsmr run root//foo/bar:bar -- -test.bench=.
```
</FbInternalOnly>

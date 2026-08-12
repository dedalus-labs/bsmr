---
id: toolchains
title: Toolchains
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->


# Toolchains

## Native frontend setup

Native Go repositories should let Bessemer select and acquire the toolchain:

```shell
bsmr go toolchain
bsmr go toolchain --check
```

The command selects the latest stable Go release when no lock exists and
otherwise acquires the committed release. `--update` moves an existing lock to
the latest stable release; `--version` selects an exact release such as `1.26`
or `1.26.5`. It
commits the official archive names, SHA-256 digests, and byte lengths for
supported hosts, then materializes and verifies the current host SDK as a
repository-local ignored input. The check command is offline and rejects lock,
generated IR, acquisition-metadata, or SDK-version drift.

The committed `.bsmr-go-toolchain.json` is intentionally small. Each field
protects a different part of the toolchain contract:

| Field | Purpose |
| --- | --- |
| `generated_by` | Establishes ownership before Bessemer overwrites generated configuration; it is not an artifact digest. |
| `schema` | Prevents a lock written with newer semantics from being accepted by an incompatible reader. |
| `version` | Fixes the Go language, package-selection, compiler, linker, and standard-library behavior. |
| `archives[].os` / `archives[].arch` | Selects runnable SDK bytes for the execution host, independently of the build target. |
| `archives[].filename` | Constrains the go.dev URL to the canonical archive name for the locked version and host. |
| `archives[].sha256` | Authenticates archive content and gives it a stable content identity. |
| `archives[].size` | Rejects incomplete downloads before extraction. |

Execution host and build target are separate dimensions. A Darwin arm64 runner,
for example, must execute the Darwin arm64 SDK tools even when those tools emit a
pure-Go Linux amd64 binary. The generated toolchain therefore selects the archive
from the execution platform and derives `GOOS` and `GOARCH` from the target
platform. Host-native cgo cannot cross that boundary because its C/C++ toolchain
and sysroot are host inputs.

The ignored SDK and wrapper directories each contain a
`.bsmr-metadata.json` marker with the generator identity, acquisition state,
version, host tuple, and archive digest. Bessemer requires the complete marker to
match the lock before using the tools, and requires its ownership identity before
replacing either directory.

The remaining sections document the underlying rules for custom toolchains.

Go toolchains in Bessemer come in two types: "regular" and "bootstrap".

- **Regular toolchains** are used to build Go code.
- **Bootstrap toolchains** are used to build the tools required to build Go
  code.

A minimal setup requires `:go`, `:go_bootstrap`, and `:python_bootstrap` in your
`toolchains//` cell.

Additionally, a `:cxx` toolchain is required:

- If you use CGo, you need a fully configured CXX toolchain
- If you don't use CGo, you can define it with a stub `no_toolchain` rule

Each type of Go toolchain has two implementations: "system" and "hermetic".

## Hermetic Toolchains (Recommended)

Hermetic toolchains are the recommended way to build Go code because they:

- Make builds reproducible
- Support all features of the Go rules

Hermetic toolchains consist of two rules: `go*_distr` and `go*_toolchain`.

The `*_distr` rules wrap Go distributions produced by `go tool dist`, such as
those available at [go.dev/dl](https://go.dev/dl/).

You can use either local or `http_archive` Go distributions.

### Toolchain Locations

- `@prelude//toolchains/go:go_toolchain.bzl`
- `@prelude//toolchains/go:go_bootstrap_toolchain.bzl`

### Example Configuration

Here's an example of `go_toolchain` using local Go distributions
(`go_bootstrap_toolchain` is defined similarly):

```python
load("@prelude//toolchains/go:go_toolchain.bzl", "go_distr", "go_toolchain")

# Note: selects are resolved against execution-platform
go_distr(
    name = "go_distr",
    go_os_arch = select({
        "config//os:linux": select({"config//cpu:x86_64": ("linux", "amd64")}),
        "config//os:macos": select({"config//cpu:arm64": ("darwin", "arm64")}),
    }),
    go_root = select({
        "config//os:linux": select({"config//cpu:x86_64": "path/to/go-linux-amd64"}),
        "config//os:macos": select({"config//cpu:arm64": "path/to/go-darwin-arm64"}),
    }),
    version = "1.X.Y",
)

# Note: selects are resolved against target-platform
go_toolchain(
    name = "go",
    env_go_arch = select({
        "config//cpu:arm64": "arm64",
        "config//cpu:x86_64": "amd64",
    }),
    env_go_os = select({
        "config//os:linux": "linux",
        "config//os:macos": "darwin",
        "config//os:windows": "windows",
    }),
    go_distr = ":go_distr",
    visibility = ["PUBLIC"],
)
```

For a complete example using `http_archive`, see
[examples/toolchains/go_toolchain](https://github.com/dedalus-labs/bsmr/blob/main/examples/toolchains/go_toolchain/toolchains/BUILD.bsmr).

### Advanced: Multi-platform Toolchains

If you support multiple execution platforms using standard Go distributions
(produced by `go tool dist`), you may encounter a situation where you have
multiple copies of the Go distribution with almost identical content.

The `go_distr` rule supports a non-standard layout of the Go distribution,
allowing you to maintain a single copy of the source code with `bin` and
`pkg/tool` directories containing binaries for all execution platforms you
support.

Set `multiplatform = True` on `go_distr` to enable this behavior. In this case,
the `bin` and `pkg/tool` directories must contain binaries in `goos_goarch`
subdirectories (built for the corresponding platforms).

See the example for a Go distribution supporting two execution platforms:
linux/amd64 and darwin/arm64.

```
third-party/go/1.25.1
├── api
├── bin
│   ├── darwin_arm64
│   │   ├── go
│   │   └── gofmt
│   └── linux_amd64
│       └── ...
├── ...
├── pkg
│   ├── include
│   └── tool
│       ├── darwin_arm64
│       │   ├── asm
│       │   ├── cgo
│       │   └── ...
│       └── linux_amd64
│           └── ...
└── ...
```

You can produce it by downloading the `src` distribution from https://go.dev/dl/
and running the following commands (assuming you are running on `linux/amd64`
and have Go installed on your system).

```
$ cd distr_dir/src
$ CGO_ENABLED=0 GOOS=linux GOARCH=amd64 ./make.bash
$ CGO_ENABLED=0 GOOS=darwin GOARCH=arm64 ./make.bash
$ mkdir ../bin/linux_amd64
$ mv ../bin/go ../bin/linux_amd64
$ mv ../bin/gofmt ../bin/linux_amd64
```

## System Toolchains

System toolchains are not intended for production builds but can be used for
examples and simple projects. Note that some features of Go rules might not work
with system toolchains, such as building ASM files.

### Toolchain Locations

- `@prelude//toolchains/go:system_go_toolchain.bzl`
- `@prelude//toolchains/go:system_go_bootstrap_toolchain.bzl`

### Example Configuration

```python
load("@prelude///toolchains/go:system_go_bootstrap_toolchain.bzl", "system_go_bootstrap_toolchain")
load("@prelude///toolchains/go:system_go_toolchain.bzl", "system_go_toolchain")

system_go_toolchain(
    name = "go",
    visibility = ["PUBLIC"],
)

system_go_bootstrap_toolchain(
    name = "go_bootstrap",
    visibility = ["PUBLIC"],
)
```

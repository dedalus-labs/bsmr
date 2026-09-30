<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Introduces experimental OCI image assembly from native build outputs. -->

# Build an OCI image

Build an OCI container image from your application's BSMR outputs. The image
recipe describes where the files go and how the container starts. Compilation,
layer packing, and image configuration are separate build actions, so changing
the entrypoint can reuse the same binary and packed layers.

These rules are experimental. Their attributes and provider contracts may change.
Pin the BSMR version used by your project and use the documentation for that
revision. Reuse the same installed engine across worktrees.

## Configure the tools

The engine bundles the OCI rules and their JavaScript helpers. Create an
`oci_toolchain` with a checksum-pinned `img` v0.3.22 executable and a pinned
Node distribution. BSMR acquires these tools through `http_file`, `http_archive`,
and `node_distribution` targets, so the tool bytes participate in build identity.

The [toolchain reference](https://github.com/dedalus-labs/bsmr/blob/main/prelude/oci/README.md#tools-and-imported-content)
contains a complete setup recipe. The bundled
[tool catalogue](https://github.com/dedalus-labs/bsmr/blob/main/prelude/oci/tools.json)
holds the encoder URLs and checksums. Select tools for the machine that runs
the packaging actions. For example, macOS arm64 tools can package a Linux binary
that was cross-compiled by the language adapter.

## Package a native binary

This example assumes the [Go adapter](languages/go/native.md) exposes a complete
Linux executable at `//cmd/api:bin`, and `//tools:oci` is configured as above.
Put the image recipe in `images/api/BUILD.bsmr` so the application's package
keeps its native build definition.

```python
load("@prelude//oci:defs.bzl", "oci_layer", "oci_image", "oci_layout")

platform(
    name = "linux_arm64",
    constraint_values = ["config//os/constraints:linux", "config//cpu/constraints:arm64"],
)

oci_layer(
    name = "application",
    platform = "linux/arm64",
    executables = {"/app/api": "//cmd/api:bin"},
    toolchain = "//tools:oci",
)

oci_image(
    name = "image",
    layers = [":application"],
    platform = "linux/arm64",
    entrypoint = ["/app/api"],
    user = "65532:65532",
    working_dir = "/app",
    toolchain = "//tools:oci",
)

oci_layout(name = "layout", image = ":image", toolchain = "//tools:oci")
```

```console
bsmr build //images/api:image --target-platforms //images/api:linux_arm64
bsmr build //images/api:layout --target-platforms //images/api:linux_arm64
```

The `image` target builds the binary and produces the image configuration and
manifest. The `layout` target exports a complete OCI directory and verifies its
contents. Application source edits rebuild the affected compiler actions, whose
outputs become the layer's inputs. If you change only the entrypoint, BSMR
reuses the binary and packed layer and writes new image metadata. A requested
layout export runs again to include that metadata. Registry publication is a
separate step.

Use `--target-platforms` to configure native compilation for Linux. The image's
`platform` field checks the expected platform, and the layer checks each
executable's ELF header for its processor type. Include the application's
required shared libraries and resources explicitly, then test the image in a
Linux runtime.

## Select an external builder

If your image recipe uses a Dockerfile, give `dockerfile_image` a declared build
context and a `builder` target. The bundled `managed_buildkit` adapter runs
BuildKit in a temporary container on a local Docker daemon. It returns the same
OCI image provider as native assembly, so either path can feed `oci_layout`.

Use this adapter for trusted recipes that copy local files into a scratch image
or between local stages. The worker disables networking and rejects external
base images, downloaded frontends, and Git or HTTP sources. A Dockerfile that
downloads packages during the build needs a different input contract.

See the [builder reference](https://github.com/dedalus-labs/bsmr/blob/main/prelude/oci/README.md#explicit-buildkit-selection)
for setup and its local privilege requirements. The
[builder contract](https://github.com/dedalus-labs/bsmr/blob/main/prelude/oci/README.md#builder-contract)
accepts a build specification and produces a complete OCI layout. Adding a
Docker/buildx or Buildah backend would mean implementing this boundary and
tracking the inputs that determine its output. Builder selection is part of
the recipe. BSMR runs that builder and reports its failures.

## Scope

Native layers contain regular files, Linux executables, and literal symlinks,
packed into ordinary gzip archives. Supply base images as complete, independently
acquired OCI layouts. Registry downloads and publication, multi-platform indexes,
compact layer retention, and directory-shaped runtime bundles are outside these
rules. See the reference for supported platforms and configuration inheritance.

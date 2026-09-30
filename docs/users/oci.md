<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Introduces experimental OCI image assembly from native build outputs. -->

# OCI image preview

The v0.0.10 preview packages existing build outputs into OCI images. Build your
application with its language adapter, place its output in a layer, then choose
image configuration and export a complete image directory. Unchanged compiler
and layer work uses BSMR's existing shared cache.

The rule attributes and providers are experimental and may change before the
public API is stabilized. Pin the BSMR release used by your project. Install one
engine for the machine; image packaging does not compile BSMR in each worktree.

## Configure the tools

The installed engine includes the rules and their helper modules. Create an
`oci_toolchain` using a checksum-pinned `img` v0.3.22 executable and a pinned
Node distribution. Existing `http_file`, `http_archive`, and `node_distribution`
rules acquire these tools through the normal artifact graph.

The [toolchain reference](https://github.com/dedalus-labs/bsmr/blob/main/prelude/oci/README.md#tools-and-imported-content)
contains a complete setup recipe. The bundled
[tool catalogue](https://github.com/dedalus-labs/bsmr/blob/main/prelude/oci/tools.json)
holds the qualified encoder URLs and checksums. Select tools for the execution
host; an arm64 macOS toolchain can package a Linux image.

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

The first target returns image metadata. The second exports all required
payloads into a verified OCI directory. Neither command publishes to a registry.
Changing image configuration reuses the existing layers. Changing a binary
rebuilds its affected compiler work and layer. Failed or incomplete actions
cannot become successful cache results.

The target platform configures native compilation. The image's platform field
checks that contract; it cannot convert a host executable into a Linux binary.
Include required shared libraries and resources explicitly, then qualify the
image in a Linux runtime. ELF architecture checks alone do not prove that a
program runs.

## Select an external builder

`dockerfile_image` accepts an explicit `managed_buildkit` launcher and returns
the same image provider. The current adapter uses a pinned, disposable local
worker with fresh state. Its qualified scope is trusted COPY assembly from
scratch and declared context files. External base images and Dockerfile
frontends are rejected; networking is disabled.

See the [builder reference](https://github.com/dedalus-labs/bsmr/blob/main/prelude/oci/README.md#explicit-buildkit-selection)
for setup and its local privilege requirements. There is no automatic switch
between native assembly and an external builder after an error.

The preview uses ordinary gzip layers. Compact retention, registry acquisition
and publication, multi-platform indexes, and directory-shaped runtime bundles
are future work. Exported images follow the OCI format; this preview
does not yet promise a stable BSMR programming interface.

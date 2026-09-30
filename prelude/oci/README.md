<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Describes OCI image assembly, tool inputs, and the external-builder contract. -->

# OCI assembly

Build the application with its language adapter, then package those outputs as
an OCI image. BSMR caches compilation, layer packing, and image composition as
separate actions. Changing an entrypoint reuses the binary and packed layers.
Application source edits rebuild the affected compiler actions, whose outputs
are inputs to the application layer.
A complete layout export copies and verifies the payloads alongside the image
metadata.

The rules and their provider contracts are experimental. The engine bundles the
rules and JavaScript helpers, so use the same installed binary across worktrees.
Set `BSMR_LOCAL_CACHE_DIR` to a persistent shared directory to let those
worktrees reuse matching action results.

Layers use ordinary gzip archives. Native placement accepts individual files,
executables, and literal symlinks. Compact streams, registry downloads and
publication, multi-platform indexes, deletion layers, and directory-shaped
application outputs are outside this rule set.

~~~text
native compiler outputs -> oci_layer -> layer metadata -> oci_image
                               |                            |
                          immutable blob               image metadata
                               +-----------> oci_layout <---+
                                                |
                                       complete OCI directory

declared context -> selected builder -> verified OCI image -> oci_layout
~~~

## Rules

| Rule | Result |
| --- | --- |
| oci_layer | Explicit regular files, Linux executables, and literal links in a cached gzip layer |
| oci_image | Configuration, manifest, and descriptor assembled from layer metadata |
| oci_import | One selected platform from a complete, independently acquired layout |
| oci_layout | A complete layout with independently owned, verified payload files |
| oci_context | A normalized directory assembled from declared artifacts for an external builder |
| dockerfile_image | A Dockerfile build with declared inputs and an explicitly selected builder |
| managed_buildkit | A pinned BuildKit worker with fresh state for each uncached build |

`OciLayerInfo` carries the layer metadata, blob, and platform. `OciImageInfo`
carries the image metadata and references to the payloads needed for export.
Its descriptor identifies the manifest bytes. The surrounding `index.json` and
BSMR directory have their own identities. Subtargets expose the manifest,
configuration, and layer blob.

## Native usage

Put the image recipe in its own package. A `BUILD.bsmr` file replaces native
inference for that package, so keeping it separate preserves the application's
native build definition. This example uses a complete Linux executable at
`//cmd/api:bin` and an OCI toolchain at `//tools:oci`.

~~~python
load("@prelude//oci:defs.bzl", "oci_layer", "oci_image", "oci_layout")

platform(
    name = "linux_arm64",
    constraint_values = ["config//os/constraints:linux", "config//cpu/constraints:arm64"],
)

oci_layer(
    name = "application",
    platform = "linux/arm64",
    executables = {"/app/api": "//cmd/api:bin"},
    files = {"/app/settings.json": "settings.json"},
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
~~~

~~~sh
bsmr build //images/api:image --target-platforms //images/api:linux_arm64
bsmr build //images/api:layout --target-platforms //images/api:linux_arm64
~~~

Supported image platforms are `linux/amd64` and `linux/arm64`.
`linux/arm64/v8` normalizes to `linux/arm64`.

The target-platform flag configures native compilation. The layer's `platform`
attribute checks the expected image platform, and executable files must have the
selected ELF64 machine type. Supply the runtime libraries and resources the
application needs, including a compatible libc for dynamically linked binaries.
Test the resulting image on Linux.

Plain files have mode 0644, executables 0755, and parent directories 0755.
Ownership is numeric 0:0 and timestamps are normalized. Invalid placements fail.
These include source directories, source symlinks, special files, duplicate
destinations, and whiteout names used for deletion layers.

Omit `base` to build from scratch. A supplied base must provide `OciImageInfo`
for the same platform, and its layers retain their bytes and order. `entrypoint`,
`cmd`, `user`, and `working_dir` inherit when omitted or set to `None`. An explicit
empty list or string clears the field. Setting `entrypoint` also clears inherited
`cmd` unless you set `cmd` explicitly, following Docker semantics. Environment
and label maps merge by key.

Base history retains its order. Missing history gets explicit missing-history
markers. Inconsistent history and history-only scratch bases fail validation.
OCI descriptors use the SHA-256 of the bytes they describe.

## Tools and imported content

`oci_toolchain` takes an `img` executable target and a Node executable target.
Reuse an existing verified Node distribution target when the project has one.
The img target must expose exactly one executable artifact from a digest-verified
rules_img v0.3.22 distribution. Tools run on the execution platform, which can
differ from the image platform. BSMR acquires their declared artifacts when it
executes the build.

The following `tools/BUILD.bsmr` recipe is for a macOS arm64 execution host. It
can package a cross-compiled Linux image. On another execution host, select the
matching img asset from the bundled `tools.json` and the corresponding official
Node archive and checksum. The acquisition rules download the pinned tools
and make them available to the packaging actions.

~~~python
load("@prelude//oci:toolchain.bzl", "oci_toolchain")
load("@prelude//oci:tools.json", tools = "value")
load("@prelude//toolchains/pnpm:defs.bzl", "node_distribution")

img = tools["img"]["assets"]["darwin-arm64"]
http_file(
    name = "img",
    urls = [img["url"]],
    sha256 = img["sha256"],
    executable = True,
    has_content_based_path = True,
)

http_archive(
    name = "node_archive",
    urls = ["https://nodejs.org/dist/v26.5.1/node-v26.5.1-darwin-arm64.tar.gz"],
    sha256 = "f4387df0b46556516d19abf2f2d6806481ac8368aa7f9d96bafed422a56a1d01",
    strip_prefix = "node-v26.5.1-darwin-arm64",
    has_content_based_path = True,
)
node_distribution(
    name = "node_distribution",
    root = ":node_archive",
    node_requirement = "26.5.1",
    version = "26.5.1",
)
oci_toolchain(
    name = "oci",
    img = ":img",
    node = ":node_distribution",
    version = tools["img"]["version"],
    visibility = ["PUBLIC"],
)
~~~

The constructor tracks the runtime, executable bytes, and imported helper
modules. Its helper paths come from the engine's bundled prelude. The shared
`tools.json` catalog supplies the img URLs and SHA-256 checksums. Declare tools
as artifacts so the cache key includes their bytes. A version label or a lookup
through `PATH` is insufficient to identify an executable.

Import and export verify the manifest and configuration hashes, descriptor sizes,
platform, every referenced blob, and every uncompressed layer digest. Missing or
corrupt content fails. Exports copy payloads into independently owned files so
changes to the exported layout cannot modify the source artifacts through a
shared hardlink.

Import accepts direct OCI image manifests. Nested image indexes, attestations,
and Docker media-type layouts are unsupported. Acquire bases from trusted
sources and test their unpacked contents and runtime behavior. Digest validation
checks byte integrity, while the base image still determines what files and
programs enter the container.

## Explicit BuildKit selection

Use `dockerfile_image` when the image recipe is a Dockerfile. Select
`managed_buildkit` as its builder to run BuildKit in a temporary container on a
local Docker daemon. The adapter uses the embedded `dockerfile.v0` frontend and
supports scratch and context-local stages. It disables networking, rejects
external image, Git, HTTP, and frontend downloads, and grants no insecure
BuildKit solve entitlements.

~~~python
load("@prelude//oci:buildkit.bzl", "managed_buildkit", "dockerfile_image")

managed_buildkit(
    name = "buildkit",
    node = ":node_distribution",
    docker = "docker-client",
    daemon_contract = "daemon-contract.json",
    image = "moby/buildkit@sha256:<verified-repository-digest>",
)

dockerfile_image(
    name = "external",
    context = ":declared_context",
    builder = ":buildkit",
    platform = "linux/arm64",
    toolchain = ":oci",
)
~~~

The daemon contract is JSON with `host`, `version`, `api_version`, `os`,
`architecture`, and `kernel_version` strings. The host must be a local Unix
socket. The worker image must already be present and match its repository
digest. Each uncached build starts and removes its own container, with fresh
cache state, bounded tmpfs storage, 4 CPUs, and 2 GiB memory. A BSMR cache hit
reuses the result and skips worker startup. The cache key covers the declared
context and ignore files, build arguments, tool and helper bytes, worker image
digest, and daemon contract.

The Docker worker needs privilege for mounts, so run only trusted recipes.
Disabling networking is insufficient to isolate a hostile Dockerfile. The daemon
must run Linux on the selected native architecture. Remote daemons and
cross-architecture emulation are outside this adapter's contract.

The worker applies Dockerfile ignore rules and normalizes copied context
metadata. A daemon identity mismatch fails the build. `SOURCE_DATE_EPOCH`
normalizes export timestamps, but commands that read clocks or randomness can
still produce different output. The differential test covers `COPY` assembly
from native outputs. Arbitrary `RUN` recipes need their own reproducibility tests.

## Builder contract

`dockerfile_image` accepts a `builder` target that provides `RunInfo`, an
executable command with tracked inputs. The rule invokes it with `--spec` and
`--output`. The JSON specification contains the context path, Dockerfile path,
platform, build arguments, target stage, and source date epoch. The builder must
write a complete OCI layout to the output directory.

BSMR then verifies the layout and returns `OciImageInfo`, the same provider used
by native assembly. Adding a Docker/buildx or Buildah adapter would use this
boundary. The adapter must track every tool, configuration value, and external
identity that can change its result before those results can be cached safely.
`managed_buildkit` tracks its selected tools, verifies the daemon identity, and
starts each executed build with fresh worker state. BSMR runs the selected
builder and propagates its errors. Choosing another builder is an explicit
recipe change.

## Verification

~~~sh
BSMR_OCI_IMG=/path/to/img-v0.3.22 node --test test/oci/closure.test.mjs test/oci/operations.test.mjs test/oci/image.test.mjs
pnpm run ci check license
~~~

The cache fixture compiles real Go code, checks metadata-only invalidation,
fresh-root restoration, failed retry, rejected paths/platforms, and corrupt or
missing imports. Its optional managed-worker fixture uses independent BuildKit
COPY encoding and compares filesystem/config semantics with a separate stdlib
tar reader. See [test setup](../../test/oci/README.md).

When comparing builders, measure compilation, layer packing, and verified export
on the same workload and cache state.

References: [OCI image specification](https://github.com/opencontainers/image-spec/tree/v1.1.1),
[pinned img manifest implementation](https://github.com/bazel-contrib/rules_img/blob/v0.3.22/img_tool/cmd/manifest/manifest.go),
[BuildKit source policy](https://github.com/moby/buildkit/blob/v0.32.2/docs/build-repro.md).

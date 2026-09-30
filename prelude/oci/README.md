<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines the local OCI rule contract and its current qualification boundary. -->

# OCI assembly

Build the application with its existing language adapter, then package those
outputs. A configuration edit rebuilds only image metadata. An application edit
rebuilds its affected compiler actions and layer. BSMR's existing action cache
shares the results across worktrees; these rules introduce no separate cache.

These experimental rules are scheduled for v0.0.10. Rule attributes and provider
contracts may change before the stable API. Once published, that release's engine
includes the rules and JavaScript helpers. Use the same installed binary in every
worktree; no source checkout or engine rebuild is required. Set
BSMR_LOCAL_CACHE_DIR to a persistent shared directory to reuse cached results.

Ordinary gzip archives are the initial representation. Compact streams,
registry acquisition/publication, multi-platform indexes, deletion layers, and
directory-shaped application outputs are not implemented here.

~~~text
native compiler outputs -> oci_layer -> layer metadata -> oci_image
                               |                            |
                          immutable blob               image metadata
                               +-----------> oci_layout <---+
                                                |
                                       complete OCI directory

declared context -> selected managed BuildKit -> same image metadata/closure
~~~

## Rules

| Rule | Result |
| --- | --- |
| oci_layer | Explicit regular files, Linux executables, and literal links in a cached gzip layer |
| oci_image | Config, manifest, and descriptor; composition does not read layer payloads |
| oci_import | One selected platform from a complete, independently acquired layout |
| oci_layout | A complete layout with independently owned, verified payload files |
| oci_context | A normalized directory assembled from declared artifacts for an external builder |
| dockerfile_image | A closed-input Dockerfile solve using an explicitly selected builder |
| managed_buildkit | A pinned private-worker launcher with fresh state per executed solve |

OciLayerInfo carries metadata, blob, and platform. OciImageInfo carries manifest,
config, descriptor, platform, and the artifact references required for export.
The image descriptor identifies the manifest bytes, not the wrapper index.json
or BSMR directory digest. Subtargets expose the manifest/config and layer blob.

## Native usage

Place authored image recipes in a separate package: BUILD.bsmr replaces native
inference for its entire package. The example assumes a complete Linux runtime
artifact at //cmd/api:bin and an OCI toolchain at //tools:oci.

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

The target-platform flag configures native compiler dependencies. The layer's
platform string checks the declared image contract; it does not cross-compile a
macOS executable. Executables must have the selected ELF64 machine type.
This header check does not prove libc compatibility, dynamic-library closure,
or that a binary runs. Supply the runtime libraries/resources explicitly and
qualify the resulting image on Linux. Plain files have mode 0644, executables
0755, parent directories 0755, numeric ownership 0:0, and normalized timestamps.
Source directories, source symlinks, special files, duplicate placements, and
whiteout names fail instead of selecting another packing path.

An omitted base means scratch. A present base must supply OciImageInfo with the
same platform. Base layers retain their bytes and order. Null configuration
attributes inherit; an explicit empty list/string clears the field. Setting an
entrypoint clears inherited cmd unless cmd is explicitly set, following Docker
semantics. Environment and label maps merge by key. Base history order is
preserved; absent history gets explicit missing-history markers. Inconsistent
history and history-only scratch bases fail rather than emit misleading history.
No source/toolchain hash is
relabelled as an OCI SHA-256 digest.

## Tools and imported content

oci_toolchain takes an img executable target and a Node executable target.
Reuse an existing verified Node distribution target when the project has one.
The img target must expose exactly one executable artifact from a digest-verified
rules_img v0.3.22 distribution. Tools use the execution platform, which can differ
from the image platform. No tool is downloaded or compiled during rule analysis.

The following tools/BUILD.bsmr recipe is for a macOS arm64 execution host. It
can package a cross-compiled Linux image. On another execution host, select the
matching img asset from the bundled tools.json and the corresponding official
Node archive/checksum. This uses existing acquisition rules; Node and img are
tools, so installing BSMR does not require recompiling either tool.

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
modules. Its helper paths come from the installed engine's bundled prelude.
The shared tools.json catalog supplies the exact img release hashes used in
qualification. A version label alone is not a pin. Installation/acquisition policy
remains outside this rule set; do not use ambient PATH binaries as tracked tools.

Import and export verify manifest/config hashes, descriptor sizes, selected
platform, all referenced blobs, and every uncompressed layer digest. Missing or
corrupt content fails. Exports copy payloads into fresh independently owned
files, avoiding source/output hardlink aliasing. Import accepts direct image
manifests, not nested image indexes, attestations, or Docker media-type layouts.
It verifies bytes, not the security of arbitrary imported tar filesystem changes;
acquire trusted bases and run the independent unpack/runtime qualification.

## Explicit BuildKit selection

Use dockerfile_image for a Dockerfile, never as an error fallback from oci_image.
The qualified launcher supports scratch and context-local stages only. It uses
embedded dockerfile.v0, denies external image/Git/HTTP/frontend acquisition,
disables networking, and grants no insecure BuildKit solve entitlements.

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

The daemon contract is JSON with host, version, api_version, os, architecture,
and kernel_version strings. The host must be a local Unix socket. The cached
worker image must match its exact repository digest. Each executed solve creates
and removes its own container with no persistent cache mounts, bounded tmpfs
state, 4 CPUs and 2 GiB memory. A warm BSMR action-cache hit does not start a
worker. Dockerfile/context/ignore files, arguments, runtime/client/helper bytes,
image digest, and daemon contract participate in outer action identity.

This adapter uses a privileged local Docker worker for mount capability.
Network isolation is not a hostile-Dockerfile security boundary. It is qualified
for trusted local recipes on the selected native Linux architecture, not remote
production daemons, arbitrary online Dockerfiles, Docker/buildx/Buildah adapters,
or cross-architecture emulation. The worker retains Dockerfile ignore semantics
and normalizes copied context metadata. An identity mismatch is an error.
SOURCE_DATE_EPOCH normalizes export timestamps; it does not make an arbitrary
program that reads clocks or randomness deterministic. The differential fixture
qualifies COPY-based assembly of native outputs, not arbitrary RUN behavior.

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

The earlier [packaging measurements](../../docs/developers/perf/oci_packaging.md)
measure different primitives, including hardlinked export. They do not prove an
8.3x gain for these verified-copy wrappers or full native compilation. Measure
the actual rules before extending that claim.

References: [OCI image specification](https://github.com/opencontainers/image-spec/tree/v1.1.1),
[pinned img manifest implementation](https://github.com/bazel-contrib/rules_img/blob/v0.3.22/img_tool/cmd/manifest/manifest.go),
[BuildKit source policy](https://github.com/moby/buildkit/blob/v0.32.2/docs/source-policy.md).

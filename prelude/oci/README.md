<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Describes compact native OCI layers, tool inputs, and verified exports. -->

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

Native file and directory layers retain compact streams and their original
inputs. Metadata-only builds do not retain a second full tar blob. Export
reconstructs and verifies ordinary OCI bytes. Imported archives retain their
original bytes. Filesystem execution and multi-platform indexes are outside this rule set.

~~~text
native compiler outputs -> oci_layer -> layer metadata -> oci_image
                               |                            |
                         compact stream               image metadata
                               +-----------> oci_layout <---+
                                                |
                                       complete OCI directory
~~~

## Rules

| Rule | Result |
| --- | --- |
| oci_pull | A public image acquired anonymously from a direct manifest lock |
| oci_fetch | An explicit authenticated acquisition command |
| oci_push | An explicit publication command |
| oci_layer | Files, directories, Linux executables, and literal links in a compact layer |
| oci_layer_from_tar | An unchanged tar or gzip archive with verified layer metadata |
| oci_image | Configuration, manifest, and descriptor assembled from layer metadata |
| oci_import | One selected platform from a complete, independently acquired layout |
| oci_layout | A complete layout with independently owned, verified payload files |

`OciLayerInfo` carries metadata, a platform, and either an archive blob or a
compact stream with its original inputs. `OciImageInfo`
carries the image metadata and references to the payloads needed for export.
Its descriptor identifies the manifest bytes. The surrounding `index.json` and
BSMR directory have their own identities. Subtargets expose the manifest,
configuration, compact stream, and an explicitly materialized layer blob.

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
These include source symlinks, special files, duplicate destinations, and
whiteout names used for deletion layers. A directory in `files` contributes every
entry, including empty directories and literal symlinks. Host symlink targets
are never followed. Use `oci_layer_from_tar(src = "layer.tar.gz", ...)` to
preserve an archive's ownership, modes, timestamps, and extended attributes.

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

## Acquire a locked public base

```python
load("@prelude//oci:defs.bzl", "oci_pull")

oci_pull(
    name = "base",
    image = "docker.io/library/debian:bookworm-slim",
    platform = "linux/arm64",
    lock = "image.lock.json",
    toolchain = "//tools:oci",
)
```

The lock contains exactly `version` (1), `image`, `platform`, and
`manifest_digest`. The digest must identify a direct OCI or Docker schema-2 image manifest.
Image spelling and platform must match the rule. Index locks are rejected;
select and pin the platform's child manifest explicitly.

`oci_pull` supports public anonymous registries, including ECR and GCR.
The client receives an empty Docker configuration and cannot discover cloud
credentials, metadata identity, or host credential helpers. Registry hosts must
be DNS names. IP literals, `localhost`, and `.localhost` names are rejected
because the pinned client can retry those addresses over plaintext HTTP.
All config and layer bytes are verified before an image provider is returned.
Tag refresh is not part of a cached build.

Docker schema-2 manifests with ordinary gzip layers are normalized by the
pinned `img` tool. The config bytes and compressed layer digests stay unchanged.
The manifest receives a new digest because its media types change. The returned
image descriptor identifies that normalized manifest. Unknown Docker descriptor
fields, foreign layers, and unsupported image formats fail explicitly.

## Acquire a private image

Authenticated acquisition is an explicit command. Credentials never enter a
rule attribute, declared action environment, or cached acquisition output:

```python
# acquire/BUILD.bsmr
load("@prelude//oci:defs.bzl", "oci_fetch")

oci_fetch(
    name = "base",
    image = "registry.example.com/team/base:release",
    platform = "linux/arm64",
    lock = "image.lock.json",
    toolchain = "//tools:oci",
)
```

Keep the consumer in a separate package. Its layout source does not exist until
the fetch completes, so its BUILD file cannot load during initial acquisition:

```python
# images/BUILD.bsmr
load("@prelude//oci:defs.bzl", "oci_import")

oci_import(
    name = "base",
    layout = "base",
    platform = "linux/arm64",
    toolchain = "//tools:oci",
)
```

Set `IMG_REGISTRY_AUTH_HOST` to the exact registry host, including its port if
present. Supply either `IMG_REGISTRY_AUTH_USERNAME` and
`IMG_REGISTRY_AUTH_PASSWORD`, or a ready-to-send
`IMG_REGISTRY_AUTH_BEARER_TOKEN`. Obtain short-lived credentials through your
registry's normal login process. Pass them through the runtime environment,
not command arguments or checked-in files.

```sh
bsmr run //acquire:base -- --output images/base
bsmr build //images:base
```

The output directory must not exist. The fetch validates the complete layout
before reporting success. `oci_import` then tracks its bytes as ordinary build
inputs. Keep private layouts out of source control and use a private action
cache for private-image targets. A cache hit is not a fresh
registry authorization check.

Custom registry trust roots use absolute `SSL_CERT_FILE` or `SSL_CERT_DIR` paths
in the explicit command's environment. Registries must use DNS hostnames.
IP literals, `localhost`, and `.localhost` names are rejected because the pinned
client can retry those addresses over plaintext HTTP. TLS verification stays
enabled. Do not use shell tracing or `bsmr run --command-args-file` with credentials: that option
records the calling process's environment.

## Publish explicitly

```python
oci_push(
    name = "publish",
    image = ":image",
    repository = "ghcr.io/example/service",
    tags = ["release"],
    toolchain = "//tools:oci",
)
```

`bsmr build //images:publish` prepares bytes and a publication request.
`bsmr run //images:publish` performs the registry mutation, even when preparation
came from cache. Empty tags publish by immutable digest. Credentials are supplied
only to the explicit run process, using the same host-scoped variables as
`oci_fetch`. Without explicit credentials, publication is anonymous. Ambient
cloud credentials, Docker configuration, and credential helpers are not used.

For an owned local test destination:

```sh
bsmr run //images:publish -- --sink oci:/absolute/new/test-layout
```

The local sink verifies publication preparation and complete output bytes.
It does not qualify live registry authentication or upload behavior.

## Verification

~~~sh
BSMR_OCI_IMG=/path/to/img-v0.3.22 node --test test/oci/closure.test.mjs test/oci/operations.test.mjs test/oci/image.test.mjs test/oci/compact.test.mjs
pnpm run ci check license
~~~

The cache fixture compiles real Go code, checks metadata-only invalidation,
fresh-root restoration, failed retry, rejected paths/platforms, and corrupt or
missing imports. Compact-layer tests reconstruct standard bytes and check
metadata-only operation without a retained tar blob. See
[test setup](../../test/oci/README.md).

When comparing builders, measure compilation, layer packing, and verified export
on the same workload and cache state.

References: [OCI image specification](https://github.com/opencontainers/image-spec/tree/v1.1.1),
[pinned img manifest implementation](https://github.com/bazel-contrib/rules_img/blob/v0.3.22/img_tool/cmd/manifest/manifest.go).

<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Documents qualification of OCI rules consuming native build outputs. -->

# OCI qualification

Run the narrow helper and source-directory check before the compiler fixture:

```shell
node test/oci/tools.ts /path/to/bsmr /path/to/img prelude
```

It executes a real directory layer through `oci_toolchain` and checks warm
reuse. This proves helper module adjacency and directory acceptance without
a compiler.

Run this fixture with release BSMR v0.0.9, the qualified rules_img v0.3.22
executable for the execution host, and this checkout's prelude:

```shell
node test/oci/cache.ts /path/to/bsmr /path/to/img prelude prelude/oci/operations.mjs
```

The fixture acquires the official Go 1.26.7 SDK and synchronizes its real native
build graph. It compiles a Linux arm64 executable through that graph and passes
the tracked output to `oci_layer`. Packaging does not invoke another compiler.
The fixture declares its Node executable and operations helper as tool inputs.
Imported helper modules are declared inputs too. Set `BSMR_OCI_TEST_EVIDENCE` to
an external directory to retain build reports, action logs, and the verified
native image layout after temporary workspaces are removed.
The default target is `linux/arm64`. Pass `--platform linux/amd64` on the Linux
amd64 CI lane. The target constraints select Linux plus `arm64` or `x86_64`;
the execution host independently selects its SDK tools.
`tools.ts` accepts the same platform option. A source engine for the preview
release is selected explicitly with `--engine-version 0.0.10`; the default
qualification engine remains the released `0.0.9`.

Pass `--bundled-prelude` to either fixture to qualify the OCI files actually
embedded in the engine. This mode does not copy the source prelude or disable
its bundled external cell. CI uses this mode for the source-built engine, and
the published preview must pass it too. The source checkout and matching
operations path remain required for the independent direct corruption checks;
the native image build consumes the bundle.

```shell
node test/oci/cache.ts /path/to/bsmr /path/to/img prelude prelude/oci/operations.mjs \
  --platform linux/amd64 --engine-version 0.0.10 --bundled-prelude
```

Each successful invocation emits a JSON receipt with its trace ID, local and
cached action counts, image digest, and executed action identities. It checks
that a warm build executes nothing, a config edit changes only image and layout
actions, a binary edit changes its layer, and deleted outputs or another source
root restore the same image from the independent shared cache. Invalid Go source
must fail; repairing it must recover the previously verified image.

For diagnosis, `BSMR_OCI_TEST_PRESERVE_FAILURE=1` retains a failed compiler
fixture after stopping its daemons. `BSMR_OCI_TEST_RESUME` may name that fixture
root to reuse its acquired SDK and cache. A resumed baseline is reported as
`seed-restored`, not as a successful cold build. The warm, config, changed-input,
restoration and fresh-root assertions still run.
Release qualification starts without either diagnostic variable and verifies
an explicitly empty action cache. Its initial cold build must execute real
compiler and packaging actions with zero cache hits.

`verify.py` independently reads complete OCI layouts with Python's standard
library. It checks descriptor SHA-256 and sizes, decompressed layer digests,
paths, effective file bytes and metadata, symlinks, and runtime configuration.
Whiteouts and special filesystem entries require separate qualification.
It can also compare the filesystem of a separate builder's exported layout:

```shell
python3 test/oci/verify.py /path/to/layout
```

These local checks establish output composition and local cache restoration.
They do not establish container runtime execution, external registry
publication, remote cache behavior, or guest rootfs construction. Those require
their own receipts and authorized workers.

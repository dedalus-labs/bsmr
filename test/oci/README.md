<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Documents the exact proof boundaries of native OCI integration tests. -->

# OCI qualification

Install the checkout's locked JavaScript dependencies before running fixtures:

```sh
pnpm install --frozen-lockfile --ignore-scripts
```

TypeScript fixtures use Hollywood's `nodeExec` for commands and GNU coreutils
`timeout` for deadlines. Linux CI provides coreutils; macOS needs `timeout` on
`PATH`. The registry harness retains isolated credential environments and its
own cancellable process lifetime.

The helper tests check metadata, compact streams, input validation, and failures:

```sh
BSMR_OCI_IMG=/path/to/pinned/img node --test test/oci/*.test.mjs
```

The public registry test is opt-in:

```sh
BSMR_OCI_IMG=/path/to/pinned/img BSMR_OCI_PULL_TEST=1 \
  node --test test/oci/sources.test.mjs
```

The native runtime tests require a trusted rootful Linux worker and a complete
Debian image layout. No Docker or BuildKit socket is used:

```sh
BSMR_OCI_REQUIRE_NATIVE=1 \
BSMR_OCI_PLATFORM=linux/arm64 \
BSMR_OCI_RUN_BASE=/path/to/base-layout \
BSMR_OCI_UMOCI=/path/to/pinned/umoci \
BSMR_OCI_RUNC=/path/to/pinned/runc \
  node --test test/oci/run.test.mjs
```

These tests execute a real package post-installation script and verify
permissions, numeric ownership, hard links, symlinks, deletions, immutable base
and input bytes, unchanged image configuration, cancellation, concurrency, and
cleanup. Set `BSMR_OCI_PLATFORM=linux/amd64` on an amd64 worker. The selected
platform, Node architecture, and Linux kernel architecture must agree; emulated
execution is rejected. Without `BSMR_OCI_REQUIRE_NATIVE=1`, the native cases are
skipped. With that flag, missing root privileges, tools, or base inputs fail.

## Actual BSMR graph

For the native Debian pipeline:

```sh
BSMR_OCI_TEST_EVIDENCE=/absolute/evidence \
  node test/oci/run.ts /path/to/bsmr /path/to/img /path/to/umoci /path/to/runc \
  /path/to/base-layout /path/to/debian.lock.json prelude
```

The lock must be APT-resolved for curl and ca-certificates against that exact
base. The graph acquires those archives through BSMR download actions, installs
them offline with `oci_run`, executes curl, checks the generated certificate
bundle, and queries dpkg. It verifies cold/warm builds, cache restoration after
output deletion and daemon restart, changed inputs/commands, stale locks, and
corrupt package checksums.

Source-prelude mode defaults to released BSMR 0.0.9. That establishes source-rule
behavior, not a newly packaged engine. The graph fixture accepts
`--engine-version` and `--bundled-prelude` for a source-built engine:

```sh
BSMR_OCI_TEST_EVIDENCE=/absolute/evidence \
  node test/oci/run.ts /path/to/source-built/bsmr /path/to/img /path/to/umoci /path/to/runc \
  /path/to/base-layout test/oci/fixtures/linux-amd64.debian.lock.json prelude \
  --platform linux/amd64 --engine-version 0.0.10 --bundled-prelude
```

Bundled mode expands the engine's actual embedded prelude, compares every
regular file's bytes and executable bit with the selected source, then removes
the inspection copy before building. A matching version string alone is not
enough. The receipt records the bundle hash, tool hashes, architecture, and
kernel release. `test/oci/fixtures/linux-amd64.image.lock.json` identifies the
amd64 base; arm64 uses the locks in `examples/oci/debian`.

The complete checked-in Linux arm64 example has its own acquisition proof:

```sh
node test/oci/example.ts /path/to/bsmr /path/to/repository /absolute/evidence
```

It copies `examples/oci/debian`, acquires its actual pinned tools, builds the
runtime check, verifies zero actions when warm, and saves the exported OCI tar.
It also accepts `--engine-version 0.0.10 --bundled-prelude` to verify the embedded
prelude before running the checked-in example.

## Native Go composition

The small helper fixture accepts files and directories without a compiler:

```sh
node test/oci/tools.ts /path/to/bsmr /path/to/img prelude
```

The compiler fixture builds a real Linux Go executable and packages its output:

```sh
BSMR_OCI_TEST_EVIDENCE=/absolute/evidence \
  node test/oci/cache.ts /path/to/bsmr /path/to/img prelude prelude/oci/operations.mjs
```

The default platform is Linux arm64 and default engine version is 0.0.9.
Pass `--platform linux/amd64 --engine-version 0.0.10 --bundled-prelude`
to test a matching embedded engine bundle. Source and matching helper paths
remain required for independent corruption checks.

The fixture verifies configuration and binary edits, failed compilation,
restoration across output deletion and a fresh source root, and corrupt/missing
content. It uses the official Go 1.26.7 SDK. `BSMR_OCI_TEST_RESUME` and
`BSMR_OCI_TEST_PRESERVE_FAILURE` are diagnosis-only; a resumed run is not a
cold-cache proof.

## Private registry round-trip

Use only a disposable, rootful Linux worker. The harness installs a temporary
fixture CA and hosts entry, runs the pinned Distribution registry over verified
TLS, then removes its trust changes and processes. It requires openssl,
apache2-utils, and ca-certificates. Download the architecture-specific archive
listed in `registry.json`; the harness verifies its SHA-256 before execution.

```sh
node test/oci/registry.mjs \
  --bsmr /path/to/bsmr --img /path/to/img \
  --registry-archive /path/to/registry.tar.gz \
  --layout /path/to/base-layout --prelude prelude \
  --bind WORKER_IPV4 --evidence /absolute/evidence
```

The bind address must belong to this worker and must not be loopback. The test
prepares real `oci_fetch` and `oci_push` targets without contacting the registry,
publishes with explicit credentials, removes its own manifest, restores cached
preparation, and explicitly republishes. It fetches and imports the image again,
checks its digest and contents, rejects missing/wrong credentials, checks logs
for credential leaks, and rejects anonymous access to the private image.
Add `--engine-version 0.0.10 --bundled-prelude` for a source-built engine.

## What the evidence does not prove

Local receipts qualify Linux arm64 native execution and same-worker local
restoration using a source-prelude overlay. Native amd64 and current bundled
engine qualification require their own completed hosted-run receipts.
Rootless execution, cross-host reproducibility, and remote cache upload of
filesystem commands are not qualified. The private-registry harness qualifies
real TLS authentication and upload against a disposable registry. It does not
prove access to a particular external registry account or cloud credential helper.

`verify.py` independently checks ordinary exported images. Native whiteout
and full filesystem semantics are tested by real `umoci` unpacking in the
runtime suite. A successful helper test is not a published binary, deployment,
or production acceptance receipt.

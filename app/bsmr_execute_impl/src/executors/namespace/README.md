<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Documents the verified runtime snapshot and its trust boundary. -->

# Namespace runtime

The runtime loader verifies and copies the launcher and root filesystem before
an executor can use them. The caller supplies a trusted launcher digest
independently of the manifest. A project cannot authorize a different launcher
by changing both the file and its recorded digest.

```text
trusted launcher digest + manifest + pinned files
    -> verify bytes while copying
    -> private launcher and root filesystem
```

The JSON manifest maps exactly two names, `bubblewrap` and `rootfs`, to the
existing `BundleArtifact` shape: `path` and `sha256`. Each path names one file
beside the manifest. The launcher must match the caller's trusted digest.
Both files must match their recorded SHA-256 digests.

The root filesystem is an uncompressed tar archive containing regular files
and directories. Links, duplicate entries and non-relative paths are rejected.
The packager must copy selected symlink targets as regular files. Each pinned
file and the extracted contents are limited to 256 MiB. The archive is limited
to 50,000 entries.

| Interface | Contract |
| --- | --- |
| `Runtime::load` | Verify pins and retain a private snapshot. |
| `Runtime::launcher` | Return the verified launcher path. |
| `Runtime::root` | Return the root filesystem for a read-only mount. |
| `Runtime::digest` | Return an identity derived from both file digests, independent of host paths. |

The snapshot lasts until its `Runtime` owner is dropped. Runtime verification
does not itself isolate a process or provide resource and cancellation limits.

```console
cargo build --locked -p bsmr_execute_impl
cargo test --locked -p bsmr_execute_impl executors::namespace::runtime::tests
```

The executor will reuse the declared input and output contracts in
[Firecracker execution](../firecracker.rs) and the
[Bubblewrap namespace interface](https://github.com/containers/bubblewrap/blob/main/bwrap.xml).

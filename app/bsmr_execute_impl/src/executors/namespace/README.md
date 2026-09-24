<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Documents namespace execution, verified runtime snapshots and their limits. -->

# Namespace runtime

Select the namespace backend explicitly and provide its runtime manifest:

```ini
[sandbox]
backend = namespace
runtime = /absolute/path/runtime.json
```

Then run `bsmr build --sandbox TARGET`. The launcher catalog supports Linux
aarch64 and x86-64 with Ubuntu Bubblewrap 0.9.0-1ubuntu0.3. The host must permit
unprivileged user, mount, PID, network, IPC and UTS namespaces.

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
file and the extracted contents are limited to 1 GiB. This admits complete
native compiler distributions with their headers and build utilities. Verification
streams through a 64 KiB buffer rather than loading the archive into memory.
The archive is limited
to 50,000 entries. The loader creates mount directories at `/workspace`, `/tmp`,
`/dev` and `/proc` before the root becomes read-only.

| Interface | Contract |
| --- | --- |
| `Runtime::load` | Verify pins and retain a private snapshot. |
| `Runtime::launcher` | Return the verified launcher path. |
| `Runtime::root` | Return the root filesystem for a read-only mount. |
| `Runtime::digest` | Return an identity derived from both file digests, independent of host paths. |

The snapshot lasts until its `Runtime` owner is dropped. Each action receives
the runtime as its read-only root and a verified copy of its declared inputs
at `/workspace`. Input files stream directly into a private tree through the
same digest verifier used by VM transport. There is no intermediate tar archive.
Executable modes and modification times retain the transport's normalized values.
The executor mounts private writable directories at output
parents and the declared scratch path. Input files and trees beneath these
parents receive read-only mounts. Input and output artifacts cannot overlap.
An input symlink that would lie in a writable directory is rejected.

The action receives its declared environment plus fixed `PATH`, `HOME` and
temporary-directory defaults. It cannot inherit the daemon environment or use
persistent workers, incremental outputs, local resources or host networking.
No host library directories are mounted. Programs such as `/usr/bin/env`
resolve inside the verified runtime. A fresh read-only `/proc` mount exposes
only the action's PID namespace. Rust's linker uses `/proc/self/exe` to find
its executable. Process root links refer to the action's filesystem. Standard
streams are the executor's capture pipes. Host processes are not exposed.

Namespaces isolate filesystem inputs and process lifetime. They do not make
kernel observations deterministic. Clocks, randomness and procfs resource
statistics are not declared file inputs. Build tools must not use those values
to select different output contents for an otherwise identical action.

Bubblewrap owns the PID namespace and terminates its descendants when the
action finishes or the existing local process runner cancels it. Only validated
declared outputs move back into the project. Output ancestor symlinks and links
that escape a declared output root are rejected before import.

The existing local scheduler and optional cgroup controls remain responsible
for resource allocation. This backend does not impose its own aggregate CPU,
memory, process-count or live disk quota. Namespace inputs retain the
100,000-node and 128-component path limits, but do not use the VM input device's
1 GiB byte ceiling. Input storage comes from the worker's filesystem and a
failed transfer prevents execution. Imported outputs retain the 1 GiB limit.
Output validation runs after execution and does not cap live writes.

Runtime digests and canonical execution properties participate in DICE reuse,
local dependency-file reuse and action-cache identity. Policy semantic changes
must bump `declared-inputs-v2`. Host paths and temporary snapshot names do not
participate in this identity.

## Native Rust package code

Configured Cargo graphs admit procedural macros and build scripts only with the verified
`declared-inputs-v2` namespace profile. They use the inherited Rust library and
macro-alias and build-script rules with the selected compiler. Native link metadata
dependencies and unimplemented script directives produce explicit errors.

The planner exports Cargo's resolved profile and target configuration. Scripts
receive literal package metadata, declared source files and `NUM_JOBS=1`.
Each script owns a copied working directory and `OUT_DIR`. Consumers compile
that returned source tree, so script changes remain isolated from the checkout.
Compiler probes receive the selected standard library. Workspace-wide Git
context is not included in the package source tree.

Graph analysis depends on the same verified execution identity as action reuse.
Changing back to host execution rejects the macro graph before an earlier
isolated result can be reused. The runtime must contain Python, the linker
driver and its libraries, plus `tar` and `gzip` for compiler archive extraction.

The real Cargo regression covers declared macro inputs, edits, warm builds,
reuse in a new checkout, rejection of host execution after a cached isolated
build, and repeated rejection of undeclared external reads:

```console
python3 test/rust/macros.py target/debug/bsmr /absolute/path/runtime.json
python3 test/rust/scripts.py target/debug/bsmr /absolute/path/runtime.json
```

## Verification

```console
cargo build --locked -p bsmr_execute_impl
cargo test --locked -p bsmr_execute_impl executors::namespace::runtime::tests
python3 test/sandbox/namespace.py target/debug/bsmr /absolute/path/runtime.json
```

The executor reuses the declared input and output contracts in
[Firecracker execution](../firecracker.rs) and the
[Bubblewrap namespace interface](https://github.com/containers/bubblewrap/blob/main/bwrap.xml).

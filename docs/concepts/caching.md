---
description: How Bessemer identifies, stores, restores, and invalidates build work.
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines Bessemer's caching and current hermeticity boundary. -->

# Caching and hermeticity

Bessemer caches actions, not commands by name. An action key identifies the
declared inputs and semantics that can affect one execution.

## Action identity

An ecosystem frontend must place all relevant state into the action graph:

- source, generated, embedded, and dependency artifacts;
- exact compiler, linker, package-manager, and SDK identities;
- target and execution platforms;
- environment variables and flags with semantic effect;
- rule implementation and configuration; and
- output declarations.

If two actions have the same key, Bessemer may reuse the stored result. If any
semantic input changes, the key changes. A cache hit is therefore a correctness
claim, not merely a timestamp shortcut.

## Storage and restoration

Outputs are stored by content digest in the CAS. Action results refer to those
digests instead of duplicating output bytes. This gives Bessemer three useful
warm paths:

1. **No-op analysis:** DICE determines that the requested graph has not changed.
2. **Action-cache reuse:** analysis produces a known action key and reuses its
   result without running the tool.
3. **Output restoration:** a known result exists while its workspace output has
   been deleted, so Bessemer materializes the CAS object without recompiling.

Remote caches speak the same action-cache and CAS model. Cache provenance is
only portable when toolchain, platform, environment, and input identities are
portable too.

After an action finishes, its outputs can move from a temporary execution path
to a path derived from their content. Cache publication reads the finalized
files. The result still records the action's original output names so another
checkout can restore them. A later execution may reuse the temporary path
without removing bytes that are still being published.

Restoration copies cached objects into writable outputs by default. Set
`BSMR_LOCAL_CACHE_MATERIALIZATION=reflink` to require filesystem copy-on-write
clones. Unsupported cloning fails the build. Modified outputs cannot change
the stored objects in either mode.

Processes sharing the local cache coordinate misses through 4096 process-lock
files. One process executes a cacheable action and publishes its result; waiting
processes restore it. Waiting releases the I/O permit, and process exit releases
the action lock. Different actions sharing a lock shard may execute in sequence.

## What “hermetic” currently means

For supported pure-Go actions, Bessemer declares exact repository inputs, an
exact verified SDK, explicit environment state, and no network requirement.
That is a hermetic input and toolchain contract.

It is not yet an enforced local isolation boundary. Bessemer does not currently
sandbox every action, so an incorrectly authored action could read an
undeclared host path. The project will not claim that stronger property until
the executor can prevent the read.

Host-native cgo has an additional boundary: the system C/C++ toolchain and
sysroot must become verified inputs before its results can be shared as fully
hermetic artifacts.

## Performance claims

Cold builds, no-op builds, private implementation edits, public API edits,
deleted-output restoration, cross-worktree reuse, and remote-cache reuse are
different workloads. Bessemer benchmarks them separately and rejects a timing
sample when the compared tools did not build equivalent outputs.

See the repository's
[benchmark contract](https://github.com/dedalus-labs/bsmr/blob/main/docs/developers/perf/benchmarking.md)
for the reproducible methodology.

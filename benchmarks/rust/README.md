<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines the workload and interpretation of the native Rust graph benchmark. -->

# Rust graph discovery

Compare two binaries on the same 128-crate workspace with 16,512 Rust files:

```sh
uv run --no-project benchmarks/rust/graph.py \
  --before /path/to/baseline/bsmr --after /path/to/candidate/bsmr \
  --cargo /path/to/1.97.1/bin/cargo
```

Both binaries must support the native Rust frontend. The matching rustup
toolchain must be installed. `--channel` changes the exact manifest pin.
The default is six samples per binary. `--runs` must be at least three.
Order alternates within each pair. Every sample starts a fresh daemon and must
resolve exactly `root//p127:p127`. No compiler actions or remote executors run.

The printed directory retains the fixture, individual logs, executable hashes,
samples and medians in `results.json`. Filesystem caches stay warm. This measures
target discovery, not compilation, physical cold storage or CI startup.

## Measured snapshot change

On Linux ARM64 (kernel 7.0.0-1019-nvidia), using unoptimized development
binaries and Cargo 1.97.1, six alternating pairs measured:

| | Before | After |
|---|---:|---:|
| Median discovery | 2.821 s | 2.569 s |
| Range | 2.578-2.936 s | 2.500-2.628 s |
| Staged snapshot files | 16,643 | 259 |

The median reduction is 8.9%. A separate snapshot-only probe measured
284 ms to 68 ms. Its result does not describe the full command.
Rust source contents do not participate in Cargo target discovery, so the
snapshot now retains conventional entrypoints. Cargo reads explicit target
paths directly from manifests without needing placeholder files.
An actual Cargo metadata comparison checks libraries, custom paths, nested
binaries, examples, tests, benchmarks and build scripts for equivalence.

Baseline executable SHA-256:
`8776f3a7b2802fc5358964612ba8f5129e2fc465266feffea58c662dcaee55b8`.
Candidate executable SHA-256:
`0fc102c84832aaa753e6650ba76eb4cc422069d9303c8048c861535b5e838ac7`.

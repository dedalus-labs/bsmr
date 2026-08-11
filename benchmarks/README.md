<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Documents Bessemer's checked, reproducible performance experiments. -->
# Benchmarks

Benchmarks are executable specifications: each suite defines its workload,
competitors, cache state, expected work, and correctness gates in the repository.
Runs write immutable fixtures, raw logs, environment metadata, individual samples,
and medians to a unique directory outside the checkout.

## Task orchestration

The orchestration suite compares BSMR with Nx 23.1.1 and Turborepo 2.10.9 on the
same generated 33-package DAG:

```text
shared -> 8 core packages -> 16 libraries -> 8 applications
```

Every task runs the same Node program, hashes its source and dependency results,
performs 25,000 SHA-256 iterations, emits one JSON result, and appends its package
name to an execution trace. The runner rejects a sample unless all of these hold:

- The observed execution count matches the invalidated graph cut.
- All 33 outputs exist after the build.
- BSMR, Nx, and Turborepo produce the same logical output digest.
- Cache restoration executes no workload tasks.

The measured regimes are warm no-op, leaf edit, shared-root edit, unrelated docs
edit, and output restoration. Each reported median contains at least three samples.
The untimed setup provisions the exact Nx and Turborepo versions; dependency
installation is intentionally outside this task-orchestration benchmark.

### Run it

Build the binary under test and start a standard REAPI action-cache/CAS service.
For example, the suite is verified with `bazel-remote` 2.6.2:

```shell
cargo build --release -p bsmr --bin bsmr
bazel-remote --dir /tmp/bsmr-bazel-remote --max_size 10 --http_address 127.0.0.1:8088 --grpc_address 127.0.0.1:9092
```

In another terminal, run:

```shell
BSMR_BENCH_BINARY="$PWD/target/release/bsmr" node benchmarks/orchestration/run.ts
```

The command prints the absolute path to `results.json`. Raw command output is next
to it under `logs/`; generated fixtures remain available for inspection.

The following environment variables configure one explicit implementation path:

| Variable | Default | Contract |
| --- | --- | --- |
| `BSMR_BENCH_BINARY` | required | Absolute or checkout-relative BSMR binary |
| `BSMR_BENCH_CONCURRENCY` | logical CPU count | Shared maximum task parallelism |
| `BSMR_BENCH_REMOTE_CACHE` | `grpc://127.0.0.1:9092` | BSMR REAPI cache endpoint |
| `BSMR_BENCH_ROOT` | platform temporary directory | Parent for immutable run directories |
| `BSMR_BENCH_RUNS` | `3` | Samples per regime; values below three are rejected |

### Interpretation boundary

This suite measures graph scheduling, invalidation, task-result caching, and output
restoration after setup. It does not measure package resolution or dependency
installation. BSMR restores from the configured REAPI CAS after `bsmr clean`,
including daemon restart; Nx and Turborepo restore from their local task caches.
Those are each tool's configured cache semantics, but backend latency must be
reported when comparing restoration numbers across machines.

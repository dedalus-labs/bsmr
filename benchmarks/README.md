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
installation. BSMR restores missing leaves from the configured REAPI CAS while
retaining its daemon; Nx and Turborepo restore from their local task caches.
Backend latency must be reported when comparing restoration numbers across
machines.

## Native package API

The native API suite generates a zero-build-file pnpm workspace and compares
`bsmr targets apps/api` with the explicit-label control
`bsmr targets root//apps/api:api`. Alternating paired samples cancel ordering
drift, both commands must resolve the same target, and the run fails when the
median native-path overhead exceeds one millisecond.

```shell
BSMR_BENCH_BINARY="$PWD/target/release/bsmr" node benchmarks/native-api/run.ts
```

`BSMR_BENCH_RUNS` configures the paired sample count and must be at least 15.
`BSMR_BENCH_MAX_NATIVE_OVERHEAD_MS` can tighten the default one-millisecond
regression budget; raising it invalidates comparisons with the checked-in gate.

## Python conformance

The Python conformance suite builds one prepared repository through BSMR and
through BSMR's exact digest-pinned uv and CPython toolchains. It rejects any
difference in installed distributions, versions, wheel tags, entry-point
declarations, executable bits, or installed file contents. It then probes the
same requested imports plus a canonical missing import in both environments.
Optional first-party builds and repository tests extend the same gate beyond
third-party installation.

Both paths first materialize and compare `pylock.build.toml`, then use that
identical closure with build isolation disabled for source distributions and
first-party PEP 517 builds. On Darwin, the uv control receives BSMR's canonical
deployment target and platform shim, preventing the host kernel version from
changing wheel tags.

Installer bookkeeping, bytecode, absolute script shebangs, and uv's executable
trampoline are the only normalized differences. Source directories are build
inputs, never import roots, so a passing import cannot hide a broken wheel by
loading the checkout directly.

The uv control builds first-party projects from a filtered immutable copy. It
preserves declared source and Git metadata while excluding virtual environments,
installer metadata, caches, and build outputs. The benchmark therefore cannot
mutate the repository under test or pass because of state left by an earlier
backend invocation.

```shell
BSMR_BENCH_BINARY="$PWD/target/release/bsmr" \
BSMR_BENCH_REPOSITORY=/path/to/pydantic-ai \
BSMR_BENCH_PYTHON_PROJECT_ENVIRONMENT=root//:__bsmr_python_workspace_environment \
BSMR_BENCH_PYTHON_SOURCE_ROOTS=".,pydantic_ai_slim,pydantic_graph,pydantic_evals" \
BSMR_BENCH_PYTHON_IMPORTS="pydantic_ai,pydantic_graph,pydantic_evals" \
node benchmarks/python-conformance/run.ts
```

The command writes raw semantic snapshots and `results.json` beneath a unique
temporary run directory. The report identifies the machine, target platform,
cache state, closure size, exact BSMR, CPython, and uv binaries, BSMR's first,
warm, and resident no-op builds, and each uv build phase. Configuration is
explicit:

| Variable | Default | Contract |
| --- | --- | --- |
| `BSMR_BENCH_BINARY` | required | BSMR binary under test |
| `BSMR_BENCH_REPOSITORY` | required | Prepared repository root |
| `BSMR_BENCH_PYTHON_LOCK` | `pylock.toml` | PEP 751 runtime lock |
| `BSMR_BENCH_PYTHON_BUILD_LOCK` | `pylock.build.toml` | PEP 751 build lock |
| `BSMR_BENCH_PYTHON_ENVIRONMENT` | root runtime environment | BSMR dependency target |
| `BSMR_BENCH_PYTHON_BUILD_ENVIRONMENT` | root build environment | BSMR build-dependency target |
| `BSMR_BENCH_PYTHON_PROJECT_ENVIRONMENT` | unset | Optional BSMR first-party environment target |
| `BSMR_BENCH_PYTHON_SOURCE_ROOTS` | `.` | Comma-separated projects built by uv |
| `BSMR_BENCH_PYTHON_IMPORTS` | empty | Comma-separated modules that must import |
| `BSMR_BENCH_PYTHON_TEST_TARGET` | unset | Optional BSMR test target |
| `BSMR_BENCH_PYTHON_TEST_COMMAND` | unset | Matching uv-side JSON argv |
| `BSMR_BENCH_ISOLATION_DIR` | repository default | Explicit BSMR output and daemon isolation used for controlled cold runs |
| `BSMR_BENCH_CACHE_STATE` | repository local state preserved | Explicit BSMR cache-state label recorded in the report |
| `BSMR_BENCH_ROOT` | platform temporary directory | Parent for immutable run directories |

The test target and uv-side command are an inseparable pair. Supplying only one
fails before execution. The suite records timings for diagnostics; performance
regression gates remain separate benchmarks so correctness cannot be traded for
a faster but semantically different installation.

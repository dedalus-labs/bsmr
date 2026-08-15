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

### Pinned real-world corpus

The corpus runner turns the RFC 0004 adoption repositories into one repeatable
gate. It fetches exact commits of NVIDIA Cosmos Cookbook, Dedalus Agents Python,
and Pydantic AI; exports their existing `uv.lock` files without resolution;
authors universal, cutoff-bounded PEP 751 build locks; and applies only BSMR's
root configuration. Mutable default branches and ambient Python tools never
enter the result.

First obtain BSMR's pinned uv and CPython output paths from any prepared Python
repository, then create and run the corpus:

```shell
bsmr build root//:__bsmr_uv_distribution root//:__bsmr_python_distribution \
  --show-full-json-output

BSMR_BENCH_CORPUS_ROOT=/tmp/bsmr-python-corpus \
BSMR_BENCH_UV=/absolute/path/from/the/json/uv \
BSMR_BENCH_PYTHON=/absolute/path/from/the/json/python3 \
  node benchmarks/python-conformance/prepare.ts

BSMR_BENCH_CORPUS_ROOT=/tmp/bsmr-python-corpus \
BSMR_BENCH_BINARY="$PWD/target/release/bsmr" \
  node benchmarks/python-conformance/run-corpus.ts
```

The 2026-08-15 Apple M5 Max reference run passed all three repositories with
CPython 3.14.7 and uv 0.12.5. The gate compared 110 runtime distributions,
7,776 runtime files, 17 build distributions, 418 build files, six first-party
wheels, the requested imports, entry-point metadata, executable bits, missing
import failures, and complete installed-file digests. Pydantic AI alone covered
a four-project uv workspace, 96 runtime distributions, and 7,462 files. Its
resident BSMR no-op completed in 36 ms.

## Python build systems

The Python build-system suite compares BSMR's zero-build-file project graph with
Bazel 9.2.0 and rules_python 2.3.0 on Django commit
`3436cf9bce84bb1f6877ad96819637366b27b719`. Bazel receives an explicit,
hand-tuned `MODULE.bazel` and `BUILD.bazel`; BSMR consumes Django's native
`pyproject.toml` plus PEP 751 locks.

The harness refuses to report timings unless both systems:

- run the expected Django entry point and import smoke test;
- produce the same 3,688-file `django/` wheel payload by path, size, and CRC;
- produce the same eight-file Git-derived `.dist-info` tree, including RECORD;
- preserve BSMR's declared source artifact without backend or bytecode writes;
- observe every leaf edit through the built entry point and test process;
- change only `django/views/generic/base.py` after the leaf edit; and
- reproduce the final edited wheel after its materialized tree is deleted.

It measures acquisition from empty tool, repository, and action caches;
provisioning with downloaded artifacts but no action results; a fresh checkout
with a shared action cache; resident no-op; first and cached test execution;
source-backed runtime and test leaf edits; a full PEP 517 wheel rebuild; Bazel's
non-equivalent archive-only wheel lower bound; and output restoration. Runner
order alternates on every paired sample. Cold acquisition has three samples;
every other regime has five.

Every semantics-matched regime is a checked release gate. The budgets require
BSMR to beat Bazel for acquisition, provisioned cold builds, fresh-checkout
shared cache, no-op, first and cached tests, source-backed runtime edits, and
restoration. Edited test execution must remain within 10%; the full wheel
rebuild must remain within 20%. Those phases execute the same Python workload
or declared backend, so parity rather than an invented multiplier is the honest
build-system contract. Bazel's `py_wheel` result remains an informational lower
bound because it archives sources without executing that backend.

Build BSMR, download the exact Bazelisk 1.29.0 binary for the host, verify its
SHA-256 against `config.ts`, and prepare the pinned source checkout:

```shell
cargo build --release -p bsmr
BSMR_BENCH_REPOSITORY=/tmp/bsmr-python-django \
  node benchmarks/python-build-systems/prepare.ts
```

Then run the matrix:

```shell
BSMR_BENCH_REPOSITORY=/tmp/bsmr-python-django \
BSMR_BENCH_BINARY="$PWD/target/release/bsmr" \
BSMR_BENCH_BAZELISK=/absolute/path/to/bazelisk \
  node benchmarks/python-build-systems/run.ts
```

The command prints the absolute `results.json` path. `BSMR_BENCH_ROOT` selects
its parent directory, `BSMR_BENCH_COLD_RUNS` changes the acquisition sample
count with a minimum of one, and `BSMR_BENCH_RUNS` changes the other sample
counts with a minimum of five.

### Reference result

The 2026-08-15 reference run used an Apple M5 Max with 18 logical CPUs and 48
GiB of memory on Darwin 25.5.0. Bazel used its rules_python 2.3.0 catalog pin,
CPython 3.14.4; BSMR used CPython 3.14.7. All correctness gates passed.

| Regime | BSMR median | Bazel median | Result |
| --- | ---: | ---: | ---: |
| Empty acquisition | 8.728 s | 15.598 s | BSMR 1.79x |
| Provisioned, no action result | 8.044 s | 13.105 s | BSMR 1.63x |
| Shared cache, fresh checkout | 1.319 s | 8.300 s | BSMR 6.29x |
| Resident no-op | 46 ms | 393 ms | BSMR 8.50x |
| First test | 622 ms | 1.439 s | BSMR 2.31x |
| Cached test | 29 ms | 166 ms | BSMR 5.65x |
| Source edit, runtime target | 95 ms | 260 ms | BSMR 2.74x |
| Source edit, test execution | 478 ms | 486 ms | parity; BSMR 1.02x |
| Source edit, PEP 517 wheel | 5.490 s | 5.091 s | parity; Bazel 1.08x |
| Bazel archive-only lower bound | — | 676 ms | non-equivalent |
| Deleted wheel restoration | 42 ms | 180 ms | BSMR 4.25x |

The exact fixture contract digest was
`7dd04a5ab96b87aab372348687afcaf6127c4f0d0fd3b4ac858c3c575c7ef10b`;
the BSMR binary digest was
`323e963d809c97dea0f7fc0f08f2ce4091d0ea57b4b0aca47f2e1d3eaeddea50`.
Rerun the suite before using these numbers for another revision or machine.

### Interpretation boundary

The semantics-matched Bazel control uses a declared `rules_python` toolchain,
locked build requirements, an immutable copied source tree, and Django's actual
PEP 517 backend. Its measured action verifies complete RECORD hash and size
coverage before admitting the wheel. The separate `py_package` plus `py_wheel`
target repeats Django's distribution metadata in BUILD syntax and only archives
the selected sources. Treat that timing as a useful packaging lower bound, not
semantic parity with either PEP 517 build.

This suite evaluates local Python build correctness and performance. It does not
claim that BSMR has Bazel's mature remote-execution, sandboxing, query, or IDE
ecosystem. Claims in the Python guide are deliberately limited to the measured
native-project path.

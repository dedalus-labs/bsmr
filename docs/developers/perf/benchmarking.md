---
id: perf_benchmarking
title: Benchmarking
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->


[basics.md](basics.md) covers the perf basics shared with profiling. This
page is benchmarking-specific: variance, sample sizes, fair comparison
between two binaries.

## Effect sizes

We typically aim to detect changes down to 0.3–0.5%; anything over 1% is a large win or regression.
With absh this may require ~50 samples or more. Single-shot measurements detect ~nothing in this
range — every benchmark needs many iterations.

`absh` is the standard A/B benchmarking tool:

```bash
# See `--help` for details
bsmr build @upstream//mode/opt bsmr_build//third-party/rust:absh-absh --out /tmp/absh
```

## Per-iteration variance

| Metric                     | Stddev across runs                |
|----------------------------|-----------------------------------|
| Wall time                  | 100 ms – 1 s on a 15 s build      |
| Daemon `VmHWM` (peak RSS)  | ~50 MB on a 4–5 GB build          |
| jemalloc `allocated`       | a few MB; very stable             |

`allocated` is stable enough that small samples are usable; for `VmHWM` and
wall time the only way to detect sub-1% effects is `absh`'s
paired-difference setup across many iterations.

## absh flag reference

```sh
absh -a 'bsmr ...A...' -b 'bsmr ...B...' -i -r -m -n 30
```

| Flag             | Effect                                                |
|------------------|-------------------------------------------------------|
| `-i`             | Ignore the first iteration of each variant            |
| `-r`             | Randomize order of A and B within each iteration      |
| `-m`             | Also measure max RSS of the spawned process           |
| `-n N`           | Stop after N successful iterations                    |
| `--max-time SEC` | Mark a run as failed past this many seconds           |

Always pass `-i` and `-r`.

`-m` is a key reason to prefer `--no-buckd` for memory benchmarks
(see [basics.md](basics.md#the-process-model)). With `--no-buckd` the
spawned process *is* the daemon doing the work, so `-m` captures the
meaningful peak RSS directly. With daemon mode `-m` only sees the thin
gRPC client; for real daemon RSS you'd need
[`scripts/measure.sh`](scripts/measure.sh) to read `VmHWM` from
`/proc/<daemon-pid>/status` instead.

## Two-binary comparison

When `A` and `B` are different `bsmr` binaries:

- For `--no-buckd` benchmarks (the typical case for wall-time and peak
  RSS), each invocation is fresh — no daemon state to manage.
- For daemon-mode benchmarks (needed for retained memory), alternating
  binaries between iterations naturally kills and restarts the daemon
  via version skew, which gives you fresh DICE per iteration with no
  extra setup.

One checkout is enough either way. Worktrees only matter if you want to
keep a real daemon alive in your main checkout while a long absh loop
runs in another.

## What metric answers what

| Question                            | Metric                                                              |
|-------------------------------------|---------------------------------------------------------------------|
| "How long does bsmr take?"         | Wall time (`absh` reports it; `/usr/bin/time` works for `--no-buckd`) |
| "How much memory at peak?"          | `--no-buckd` + absh `-m`, or `VmHWM` from daemon `/proc` in daemon mode |
| "How much is the daemon retaining?" | `allocator-stats.allocated`, daemon mode only                       |
| "How much CPU?"                     | User+sys time across daemon + client + forkserver                   |
| "Is bsmr doing more I/O?"          | Page faults / `/proc/<pid>/io`                                      |

Reporting "client max RSS" via `time -v` while in daemon mode is the most
common mistake — it's nearly constant regardless of build complexity
because the client is just a gRPC shim.

## Go builds

The Go suite compares native BSMR actions with Bazel 9.2.0 and rules_go 0.62.0
on equivalent pure-Go or cgo graphs:

```text
shared -> 8 core packages -> 16 libraries -> 8 applications
```

Both runners use the same host Go release. BSMR acquires the exact verified
official SDK, while rules_go selects the host SDK. They target the same local
REAPI service under separate instances and materialize every requested binary.
The suite measures cold compilation, no-op builds, private implementation
edits, exported API edits, unrelated documentation edits, output restoration,
and remote action-cache hits from clean roots.

Every sample is rejected unless:

- all eight executables run and produce the same logical output;
- every clean checkout produces the exact final populated-cache output;
- cold and source-edit samples execute Go actions;
- no-op, documentation, and restoration samples execute no Go actions; and
- private and exported edits invalidate the same number of actions in both systems.

Build BSMR, start `bazel-remote`, and provide Bazelisk or Bazel explicitly:

```shell
cargo build --release -p bsmr --bin bsmr
BSMR_GO_BENCH_BINARY="$PWD/target/release/bsmr" \
BSMR_GO_BENCH_BAZEL=/path/to/bazelisk \
node benchmarks/go/run.ts
```

Set `BSMR_GO_BENCH_MODE=cgo` to exercise native compilation and external
linking; the default is `pure`. `BSMR_GO_BENCH_CACHE_NAMESPACE` may reuse the
untimed SDK and native-toolchain cache when those toolchains are unchanged; a
unique module path keeps measured project actions cold.

The command prints a unique `results.json` path containing all samples,
medians, action counts, output digests, host details, tool versions, and the
cache endpoint. `BSMR_GO_BENCH_RUNS` defaults to three and rejects smaller
values. `BSMR_GO_BENCH_REMOTE_CACHE` and `BSMR_GO_BENCH_ROOT` select the REAPI
endpoint and output parent.

This suite measures graph orchestration, Go and optional host-native C
compilation and linking, invalidation, and cache restoration after SDK and
toolchain priming. It does not measure module resolution, SDK download,
cross-compilation, remote execution, or sandbox overhead.

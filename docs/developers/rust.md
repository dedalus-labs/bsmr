<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines the acceptance criteria for native Cargo project builds. -->

# Rust beta qualification

The beta builds an existing Cargo project, returns usable artifacts, and reuses
only results whose inputs remain valid. This page defines release gates.
The [user guide](../users/languages/rust/cargo.md) records implemented support.

## Interface contract

The intended entry points are directory-first:

```console
bsmr build
bsmr build ./crates/server
bsmr build ./crates/server:api
bsmr test ./crates/server
bsmr build ./crates/server -c rust.profile=release
```

| Input | Required behavior |
| --- | --- |
| No directory | Select the current Cargo package or workspace. |
| Package directory | Read its manifest and build its normal library and binary targets. |
| Workspace directory | Honor Cargo's `default-members` selection. |
| Explicit target | Select that target without compiling unrelated roots. |
| Relative or absolute path | Resolve the same project and outputs from any caller directory. |
| Nested package | Use Cargo's workspace ownership rules, including explicit workspace paths. |
| Features and profile | Preserve Cargo's default, explicit, all, and disabled-default feature semantics, profile inheritance, and package overrides. |
| Multiple requested roots | Match Cargo's joint feature resolution without duplicating shared compiler work unnecessarily. |
| Missing or conflicting input | Name the manifest, lockfile, compiler, target, or unsupported contract that prevents the build. |
| Successful build | Materialize the requested outputs and expose their absolute paths through JSON output. |

Cargo manifests remain authoritative. Require an existing lockfile and an exact
compiler identity. Preserve the original source layout and project Cargo
configuration. Ordinary Cargo projects require no generated build files or
mandatory initialization. Explicit BSMR configuration remains available for
mixed-language projects and overrides.

## Execution coverage

Reuse Cargo for planning and the existing native rules for compilation,
linking, build scripts, and tests. Each action declares its tools, files,
environment, platform, and outputs before its result can be reused.

| Capability | Acceptance witness |
| --- | --- |
| Dependencies | Local, excluded-path, registry, and pinned Git packages compile from their declared sources. Corrupted acquired bytes fail. |
| Build scripts | Generated files, cfg values, compiler environment, link directives, and `OUT_DIR` reach their correct consumers. Host and target profiles remain distinct. |
| Git metadata | HEAD, refs, index, dirty state, and linked worktrees affect results when consumed by a script. |
| Procedural macros | Generated Rust compiles. File and environment changes invalidate consumers or fail as undeclared inputs. |
| Native linking | Compiler, linker, sysroot, library, and relevant flags participate in action identity. Configured linkers are honored. |
| Optimization | Disabled, local, thin, and fat LTO preserve Cargo profile semantics and produce working outputs. |
| Tests | Library/binary unit tests, integration tests, custom harnesses, generated fixtures, and test-binary environment variables use the newly built artifacts. |
| Cancellation | Compiler and script descendants terminate before private outputs are removed or their ownership is released. |
| Isolation | Undeclared reads are tracked or rejected. Network access and writes outside owned outputs are rejected. Symlinks cannot bypass the boundary. |

Test both successful execution and denied operations. In particular, changing
a macro's external file from `7` to `9` must never return cached `7`. Disabling
remote cache uploads does not prevent stale local reuse.

## Milestones

1. Lock the interface cases and pinned consumer baselines. Record the current
   failure for each unsupported path before changing it.
2. Qualify compiler selection, dependency acquisition, profiles, and LTO in
   small executable fixtures. Publish independently reviewable changes.
3. Connect scripts and macros to native rules with an enforced input and
   process-lifetime contract. Qualify file, Git, environment, and escape cases.
4. Qualify native linking and complete test execution against unchanged Ruff
   and a representative service workspace. Keep private consumer details in
   private evidence.
5. Qualify the directory-first interface without initialization. Compare
   package, workspace, named-target, and joint-root behavior against Cargo.
6. Measure the performance profiles below on each supported platform. Qualify
   provider routing before enabling office builds.
7. Merge the final reviewed heads, publish through the normal release pipeline,
   and rerun acceptance using downloaded, checksum-verified release binaries.

A passing fixture, hosted check, merged commit, published archive, and working
consumer are separate receipts. A Linux service runtime requires its real
kernel, virtualization, and storage fixtures. A portable macOS test cannot
substitute for that evidence.

## Performance profiles

Compare Cargo and BSMR with identical sources, compiler, features, profile,
linker, CPU allocation, and storage. Start with two build jobs and record the
memory allocation. Keep download time separate from execution with dependencies
already acquired. Never share writable compiler state between concurrent trials.

| Profile | Required observation |
| --- | --- |
| Cold | Empty private build caches. Report acquisition, planning, compilation, linking, and materialization separately. |
| Warm, unchanged | No compiler or build-script actions execute. Report command and planning latency. |
| Unrelated edit | No selected compiler actions execute and outputs retain their identities. |
| Leaf or dependency edit | Affected outputs change. Record the action set and compare elapsed time with Cargo. |
| Profile, feature, toolchain, or linker change | Corresponding action identities change. Restoring the old inputs can reuse their validated results. |
| Deleted outputs | Restore correct bytes from the artifact cache without recompilation. |
| Fresh checkout or worker | Reuse complete matching artifacts across paths. Record cache transfer volume and materialization time. |
| Cancellation and restart | No orphan processes or partial published outputs. Restart produces valid results. |

Record wall time, peak process-tree RSS, executed/cached action counts, cache
bytes transferred, and allocated disk bytes. Preserve raw samples and tool
versions. Use three cold trials and twenty warm/edit trials after one explicit
warm-up. Report median and p95 for the repeated trials.

Initial performance targets are a warm no-op median below one second, no
more than 15% cold overhead over matched Cargo, and faster edited or restored
builds on the acceptance consumers. These are targets, not measured claims.
Investigate misses before expanding adoption. Zero stale hits and correct
output restoration are mandatory regardless of timing.

## Runner acceptance

Public workflows call the office provider **Dedalus Machines**. Try office,
Blacksmith, then GitHub capacity using the same approved payload. Preserve OS,
architecture, toolchain, memory, and storage requirements on every eligible
route. A Linux job cannot silently become a macOS qualification job.

The trusted dispatcher binds maintainer authorization to an exact source and
workflow revision. Fork payloads never execute on the office host account or
receive coordinator credentials. Native host cleanup does not provide VM
isolation. Verify registration and workload isolation before live enrollment.

Permit provider handoff only after confirming that the previous run was
cancelled without assignment. A started, failed, or ambiguously cancelled
payload is never replayed. Exercise successful office/hosted jobs, busy pools,
dispatch ambiguity, parent cancellation, payload failure, and final cleanup.
Record the actual provider, runner, revisions, queue time, and execution time.

## References

- [Cargo package and target selection](https://doc.rust-lang.org/cargo/commands/cargo-build.html).
- [Cargo build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html).
- [Rust performance harnesses](../../test/README.md).

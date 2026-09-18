---
description: Build, cache, and execution capabilities provided by Bessemer.
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- States the supported build and execution contracts with verification references. -->

# Capabilities

Bessemer builds packages from native manifests, schedules independent work,
and restores cached outputs. TypeScript, Rust, Go and Python share the build
graph and content-addressed store. Each frontend defines which inputs and
toolchains it supports. See [language support](about/language_support.md).

## Build and cache

An action key identifies declared source files, dependencies, tools,
configuration, environment and outputs. A matching cached result restores
those outputs. Editing a declared input changes the key and schedules the
affected work. [Caching and hermeticity](concepts/caching.md) explains the
identity and storage contract.

Local caching supports output restoration across checkouts. Eligible actions
can use remote action caches and content-addressed storage through the Remote
Execution API. Rust's native Cargo adapter and host-dependent native-extension
builds remain local. Cache eligibility depends on the selected frontend and
toolchain, not merely the presence of a lockfile.

## Isolated execution

`bsmr build`, `test` and `run` accept `--sandbox` on x86-64 Linux hosts with KVM
and cgroup v2. The experimental Firecracker profile gives each admitted action
a fresh networkless VM, explicit environment, declared inputs and validated
outputs. It requires cleanup before accepting the result.

The profile uses two vCPUs and 2 GiB of guest memory. It requires a provisioned,
digest-verified toolchain bundle and rejects unsupported action semantics.
Sandbox actions execute locally without action-cache lookup or upload.
Snapshots, networked actions and secrets are unsupported. See
[sandbox setup and limits](users/sandboxing.md).

Ordinary local execution does not enforce the sandbox's filesystem boundary.
A pinned dependency graph alone does not prevent undeclared reads or make a
build deterministic.

## Verify a build path

Use a BSMR binary built from the same revision as the fixtures and prelude.
These checks exercise different contracts. Passing one does not qualify the
others.

| Contract | Verification from the repository root |
| --- | --- |
| pnpm installation, compilation and cache restoration | `node test/native-typescript-cache.ts /path/to/bsmr` |
| Cargo output reuse and invalidation | `node test/native-cargo-cache.ts /path/to/bsmr` |
| Go compilation, embeds and cache eligibility | `node test/native-go-build.ts /path/to/bsmr` |
| Python dependency and runtime behavior | `python3 -m unittest discover -s prelude/python_native -p '*_test.py'` |
| Firecracker isolation and cleanup | `cargo test --locked -p bsmr_sandbox --test firecracker firecracker_conformance -- --ignored --exact --nocapture` with the bundle, launcher and fixture configured by [the KVM CI job](https://github.com/dedalus-labs/bsmr/blob/main/ci/ci.ts) |

The Python command runs unit tests for dependency and runtime assembly.
Native-extension reproducibility requires a real compiler fixture. The
Firecracker test runs real VMs and requires Linux/KVM and the operator setup.

---
description: Hermetic builds at monorepo scale.
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Introduces Bessemer and its current product boundary. -->

<div class="bsmr-hero" markdown="1">

# Bessemer

<p class="bsmr-kicker">Hermetic builds at monorepo scale.</p>

Bessemer (`bsmr`) is an open-source build system derived from
[Buck2](https://github.com/facebook/buck2). It combines an incremental
dependency graph with content-addressed execution and native ecosystem
frontends.

</div>

## What exists today

Bessemer is under active development and has not published a stable release.
The native Go frontend is the first complete ecosystem slice documented here.
It imports the package graph selected by an exact official Go SDK, then
executes the existing fine-grained Go rules through Bessemer's ordinary graph
and CAS.

The frontend is deliberately native-facing: developers keep `go.mod`,
`go.sum`, optional `go.work`, build tags, tests, and `go:embed`. Generated build
manifests are owned intermediate representation, not a second configuration
language to maintain.

## Project principles

| Principle | Contract |
| --- | --- |
| Correctness | Missing inputs and unverifiable toolchains fail the build. |
| Speed | Work is split into cacheable actions over exact dependency closures. |
| Security | Tools and archives are versioned, digest-pinned build inputs. |
| Adoption | Native ecosystem files lower into one shared build graph. |

## Current support

| Surface | Status |
| --- | --- |
| Incremental graph, local CAS, output restoration, and REAPI cache | Available |
| Native Go graph, pure-Go builds, and host-native cgo | Experimental |
| TypeScript and Node.js frontend | Active development |
| First-class Rust and Python frontends | Planned |
| Local sandboxing and remote execution | Not part of the current release boundary |

The order of first-class ecosystem investment is TypeScript, Rust, Go, then
Python. Status is stated independently: an implementation may land while its
broader product surface is still being completed.

[Build Bessemer](getting-started.md), read the [native Go guide](languages/go.md),
or inspect the [caching and hermeticity contract](concepts/caching.md).

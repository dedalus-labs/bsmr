---
id: pnpm
title: TypeScript and pnpm
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Documents the native TypeScript workspace API and its hermeticity boundary. -->

# TypeScript and pnpm

BSMR builds a pnpm workspace from its native ecosystem files. You do not need a
`BUILD.bsmr` or handwritten Starlark file for the conventional path.
BSMR reads the authoritative workspace graph, lowers it into its internal target
graph, and keeps that generated representation private.

## Workspace contract

At the repository root, commit:

- `package.json` with a non-empty `name`, `engines.node`, and exact
  `packageManager` identity;
- `pnpm-workspace.yaml` with the workspace package patterns; and
- `pnpm-lock.yaml` accepted by `pnpm install --frozen-lockfile`.

Every buildable package needs a named `package.json`, `tsconfig.json`, and
`tsdown.config.ts`. Declare internal edges with pnpm's explicit `workspace:`
protocol so BSMR and pnpm resolve the same graph.

```json title="package.json"
{
  "name": "@acme/workspace",
  "private": true,
  "engines": { "node": ">=24.0.0" },
  "packageManager": "pnpm@10.30.3+sha512.c961d1e0a2d8e354ecaa5166b822516668b7f44cb5bd95122d590dd81922f606f5473b6d23ec4a5be05e7fcd18e8488d47d978bbe981872f1145d06e9a740017"
}
```

```yaml title="pnpm-workspace.yaml"
packages:
  - apps/*
  - packages/*
```

```json title="apps/api/package.json"
{
  "name": "@acme/api",
  "private": true,
  "dependencies": {
    "@acme/core": "workspace:*"
  }
}
```

Initialize the repository once, then build by package path:

```console
bsmr init
bsmr build apps/api
bsmr build apps/api:typecheck
```

`bsmr build apps/api` emits the package with tsdown. The `typecheck` target runs
TypeScript semantic checking without emission. A package-path selector resolves
to one conventional target, while ordinary BSMR labels remain available for
queries, automation, and advanced rules.

## What BSMR owns

BSMR performs one frozen pnpm install for the repository lockfile. It does not
launch one competing installer per package. Independent compilation,
typechecking, tests, and packaging actions run above the normalized workspace
graph, where BSMR can schedule them concurrently and cache them independently.

The built-in toolchain catalog currently provides digest-pinned Node 26.5.1 on
macOS and Linux for arm64 and x86-64, plus exact pnpm 10.30.3 and 11.20.0
distributions. The root `packageManager` chooses the pnpm release; the pinned
Node runtime must satisfy `engines.node`. Unsupported versions and platforms
fail with a typed error. BSMR never consults system Node, global pnpm, or an
implicit latest-version fallback.

The install action also fails before pnpm starts unless these invariants hold:

- the configured pnpm version and SHA-512 digest exactly match
  `packageManager`;
- the lockfile exists and a frozen install accepts it;
- BSMR's `.bsmr` project-control file is absent from the install action inputs;
  and
- the declared output does not already exist.

The runner copies declared inputs into a writable action output and redirects
pnpm's store, home, Corepack home, npm cache, and user npm configuration into
action scratch space. Repository `.npmrc` files remain declared inputs. Ambient
home configuration and pnpm's global store are never consulted.

On POSIX, pnpm emits relative executable symlinks. BSMR removes pnpm metadata
that contains absolute store paths or wall-clock timestamps before admitting
the relocatable workspace to the content-addressed store. TypeScript outputs
are content addressed as well, so a warm build can restore a deleted output
without executing the compiler.

Lifecycle scripts are disabled. Packages that require install hooks remain
unsupported until those hooks become explicit sandboxed BSMR actions. BSMR
does not cache ambient code execution under a supposedly hermetic action key.

## Hermeticity boundary

The current adapter gives dependency installation a deterministic action key
and cacheable output, but a cold cache miss still allows pnpm to reach the
registry. Registry availability therefore remains outside the declared input
tree even though the frozen lockfile verifies downloaded package contents.

[RFC 0001](https://github.com/dedalus-labs/bsmr/discussions/12) specifies the
next boundary: consume the lockfile graph directly, fetch each
integrity-addressed package into BSMR's CAS, materialize the pnpm-compatible
layout without install-time network access, and promote lifecycle scripts into
separately sandboxed actions. Until then, call this the pinned pnpm adapter,
not a fully hermetic JavaScript dependency materializer.

## Custom rules

Explicit Starlark remains available when a repository needs a non-conventional
target or toolchain. An explicit build file takes precedence over native
manifest inference for that package. This is an escape hatch, not a second
implicit implementation: choosing it makes the repository's rule definition
authoritative.

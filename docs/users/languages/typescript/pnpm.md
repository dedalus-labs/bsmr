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
`tsdown.config.ts`. It must declare `typescript`, `tsdown`, and any compiler
plugins it imports as package-local dependencies. Declare internal edges with
pnpm's explicit `workspace:` protocol so BSMR and pnpm resolve the same graph.

```json title="package.json"
{
  "name": "@acme/workspace",
  "private": true,
  "engines": { "node": "^24.18.0" },
  "packageManager": "pnpm@11.20.0+sha512.9a6f330a95b66446ea088faf1521405a8a01f07fde7124cc9958dfed52d4bb436737e65b08f85f37b46fcba375092558ac51262b816844b22f63406ed166bfee"
}
```

```yaml title="pnpm-workspace.yaml"
packages:
  - apps/*
  - packages/*
useNodeVersion: 24.19.0
```

```json title="apps/api/package.json"
{
  "name": "@acme/api",
  "private": true,
  "dependencies": {
    "@acme/core": "workspace:*"
  },
  "devDependencies": {
    "tsdown": "0.22.4",
    "typescript": "6.0.3"
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

## Integration checklist

Before replacing an existing build command:

1. Run the repository's frozen pnpm install successfully.
2. Confirm `bsmr targets <package>` exposes the package and `typecheck` targets.
3. Run both targets from a clean output tree.
4. Delete the emitted output and confirm BSMR restores or rebuilds it.
5. Compare the old and new runtime contract: entry names, module format,
   executable shebangs, external imports, package manifest, and packed files.

Keep the old command authoritative until those checks agree. A successful
compiler exit alone does not prove package or executable parity.

## What BSMR owns

BSMR performs one frozen pnpm install for the repository lockfile. It does not
launch one competing installer per package. Independent compilation,
typechecking, tests, and packaging actions run above the normalized workspace
graph, where BSMR can schedule them concurrently and cache them independently.

The built-in catalog provides SHA-256-pinned Node 22.23.1, 24.18.0, 24.19.0,
26.5.1, and 26.7.0 on macOS and Linux for arm64 and x86-64. It also provides
exact pnpm 10.30.3 and 11.20.0 distributions. BSMR selects the newest catalog
runtime satisfying `engines.node`. pnpm 10's optional `useNodeVersion` selects
that exact catalog entry instead. pnpm's `nodeVersion` remains its dependency
engine-compatibility target; it does not select BSMR's runtime.

Unsupported requirements, exact pins, and platforms fail with a typed error.
BSMR never consults system Node, global pnpm, or an implicit latest-version
fallback. Per-package runtimes such as `executionEnv.nodeVersion`, and pnpm
runtime declarations for Node, Deno, or Bun, require an explicit toolchain.

The install action also fails before pnpm starts unless these invariants hold:

- the configured pnpm version and SHA-512 digest exactly match
  `packageManager`;
- the lockfile exists and a frozen install accepts it;
- BSMR's `.bsmr` project-control file is absent from the install action inputs;
  and
- the declared output does not already exist.

The runner copies declared inputs into a writable action output. It preserves
repository-relative symlinks that remain inside the workspace and rejects
absolute or escaping symlinks. It redirects pnpm's store, home, Corepack home,
npm cache, and user npm configuration into action scratch space. Repository
`.npmrc` files remain declared inputs. Ambient home configuration and pnpm's
global store are never consulted.

A native package owns every file beneath its package root except generated
trees and nested workspace packages. Put buildable applications in dedicated
workspace packages. Treating a large repository root as one emitted package is
correct but gives unrelated root files the same cache key.

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

The native adapter deliberately runs package-local `tsc` and `tsdown`; it does
not guess from `scripts.build`. Use an explicit Starlark rule for esbuild,
tsup, Vite, SWC, custom script pipelines, npm, Yarn, Bun, lifecycle hooks, or a
runtime outside the built-in catalog. The rule must declare its toolchain,
inputs, environment, and outputs.

An explicit build file takes precedence over native manifest inference for
that package. This is one authoritative implementation selected by the
repository, not a silent fallback from the native adapter.

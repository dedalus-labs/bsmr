---
title: DependencySet
description: Cross-repository dependency rules, exact locks, and release proofs.
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Explains how DependencySet rules produce exact locks and certified artifacts. -->

# DependencySet

A DependencySet is the authoritative rule graph for a build. It names the
components that may participate, the interfaces they provide and require, the
allowed compatibility ranges, target platforms, patches, and provenance rules.
It lets repositories move independently without letting an incompatible
combination enter a build or deployment.

DependencySet separates three claims:

| Claim | Example | Meaning |
| --- | --- | --- |
| Rules | `example.kernel.uapi = { min = 7, max = 7 }` | These are the combinations Bessemer may select. |
| Resolution | Exact commit plus SHA-256 tree and manifest digest | These exact bytes satisfy the rules. |
| Certification | Immutable release receipt | This locked resolution passed the required evidence. |

A Git commit is an identity, not a compatibility range. Bessemer applies ranges
to explicit integer contract versions such as a kernel ABI or guest protocol.
The lock still records the exact commit, tree digest, manifest digest, ordered
patch digests, and produced artifact digests selected for every component.

## Workflow

```console
bsmr deps verify
bsmr build apps/api
```

Normal builds verify that the committed `bsmr.lock` is one valid resolution of
the DependencySet before target resolution or build analysis begins. They do
not update the lockfile or access the network. A missing, stale, or
incompatible lock fails closed before analysis.

Changing a component is explicit:

```console
bsmr deps update kernel --catalog catalog.toml --catalog-digest sha256:...
bsmr deps update --refresh-lockfiles
bsmr deps explain
```

`bsmr deps update` orders candidates strictly by their declared release number,
selects the newest candidate satisfying the complete contract intersection, and
reports the deciding contract for every rejected candidate. Components may also
pin native ecosystem lockfiles (`Cargo.lock`, `pnpm-lock.yaml`, `go.sum`,
`pylock.toml`, or `uv.lock`) under `[[components.lockfiles]]`; `bsmr deps verify`
and `bsmr build` verify those files on disk against their pinned SHA-256
digests, while `bsmr deps update --refresh-lockfiles` explicitly rehashes them
after a native resolver update. `bsmr deps explain` prints contract
intersections, per-component reachable closure digests, and any conflicts.

## Fast local patches

Editing local source does not rewrite the DependencySet. Bessemer hashes the
changed source tree and invalidates only reachable actions. Each component's
closure digest covers only that component and its transitively required
contract providers, so updating an unrelated component leaves other closures
untouched.

External patches become shareable once their ordered patch blob digests are
recorded in `bsmr.lock`. Patch order is semantic and enters the lock identity.
Uncommitted local checkout overlays (`overlay = "..."`) are permitted during
local `bsmr build` iteration and rejected during release certification.

## Deployment boundary

Certification binds the verified DependencySet digest, exact lock digest,
canonical lock bytes, produced artifact digests, action evidence, and builder
provenance into an immutable receipt:

```console
bsmr deps receipt create \
  --artifacts artifacts.json \
  --source-commit <40-hex-sha> \
  --builder ci-runner \
  --workflow build \
  --output release-receipt.json
bsmr deps receipt verify release-receipt.json
```

`bsmr deps receipt create` fails closed if any locked component carries a local
overlay or if the output path already exists. Deployment systems verify the
receipt and deploy its exact artifact digests without running dependency
resolution or rebuilding production artifacts.

The complete design and rollout are in
[RFC 0004](../rfcs/0004-dependency-set.md). The existing
[hermetic build core](hermetic_build_core.md) explains how dependency closures
enter action keys without invalidating unrelated work.

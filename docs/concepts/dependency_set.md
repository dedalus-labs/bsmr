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
| Rules | `dedalus.lynx.uapi >=7,<8` | These are the combinations Bessemer may select. |
| Resolution | Lynx commit plus kernel SHA-256 content hash | These exact bytes satisfy the rules. |
| Certification | Immutable release receipt | This locked resolution passed the required evidence. |

A Git commit is an identity, not a compatibility range. Bessemer applies ranges
to explicit contract versions such as a kernel userspace application binary
interface (ABI) or guest protocol. The lock still records the exact commit and
content digest selected for every component.

## Intended workflow

```console
bsmr deps verify
bsmr build apps/api
```

Normal builds verify that the committed `bsmr.lock` is one valid resolution of
the DependencySet. They do not update it or resolve against a mutable registry.
An incompatible or stale lock fails before build analysis.

Changing a component is explicit:

```console
bsmr deps update lynx
bsmr deps explain
```

The update selects a new exact candidate only when all named contract ranges
remain satisfied. The resulting lock diff records the revision, tree digest,
manifest digest, patches, provided contracts, and artifact digests.

These commands describe the proposed interface. They are not implemented yet.

## Fast local patches

Editing local source does not rewrite the DependencySet. Bessemer hashes
the changed source tree and invalidates only reachable actions. Unrelated
cached work remains reusable.

External patches become shareable only after their ordered patch blobs are
recorded by digest in the lock. Release certification rejects an uncommitted
overlay, so local iteration stays quick without making an unreviewed tree
deployable.

## Deployment boundary

Build and test automation produces a receipt for the DependencySet, exact lock
digest, and output artifact digests. Deployment systems verify the receipt and
deploy those artifacts. They do not resolve a newer component or rebuild
production artifacts.

The complete design and rollout are in
[RFC 0004](../rfcs/0004-dependency-set.md). The existing
[hermetic build core](hermetic_build_core.md) explains how dependency closures
enter action keys without invalidating unrelated work.

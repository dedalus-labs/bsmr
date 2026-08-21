---
id: hermetic_build_core
title: Hermetic Build Core
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines the minimal model connecting hermetic actions, version sets, and caching. -->

# Hermetic build core

Bessemer's minimal build model is a memoized pure function:

```text
output tree = action(declared input tree)
```

Composition turns these functions into a directed acyclic graph (DAG). Three
data structures are sufficient; combining them into one abstraction would lose
useful invariants:

| Responsibility | Structure |
| --- | --- |
| Share and schedule computations | Action/artifact DAG |
| Authenticate hierarchical content | Merkle tree |
| Reuse prior results | Action-digest to output-digest map |

Bessemer already implements these structures through its action graph,
`bsmr_directory`, typed CAS digests, the Remote Execution API action format,
and local and remote action caches. Package-system work extends this model; it
does not replace it with another Merkle implementation.

## Canonical identity

For an action specification `A` and input root `I`, the cache identity is:

```text
action_digest = H(canonical_encode(A, I))
action_cache[action_digest] = output_root_digest
```

`A` contains every non-file input that can affect observable behavior,
including the executable, arguments, declared environment, output contract,
execution platform, timeout, and relevant policy. `I` is the Merkle root of
the source, generated inputs, selected dependencies, and toolchains mounted in
the sandbox.

The identity is sound only if the sandbox prevents undeclared reads, writes,
network access, and ambient environment access. The graph states the contract;
the executor enforces it. Hermeticity is therefore an execution invariant, not
a property conferred by hashing alone.

## Version sets

A version set is an immutable, content-addressed dependency universe. Each
ecosystem adapter owns its native resolution semantics and produces a canonical
Merkle DAG preserving every resolution-affecting node and edge. For pnpm this
includes source and integrity, patches, peer contexts, optionality, platform
predicates, and workspace relationships, as required by
[RFC 0001](https://github.com/dedalus-labs/bsmr/discussions/12).

Bessemer wraps the graph's canonical root record in one versioned CAS object:

```text
version_set_root = "bsmr.version-set.v1\0" || canonical_graph_root
version_set_digest = H(version_set_root)
```

The digest is the stable identity used by analysis, provenance, policy, and
queries. The root and referenced graph nodes remain available from the CAS, so
the digest is not an opaque identity disconnected from evidence. Updating one
package rewrites only its affected Merkle ancestors rather than one monolithic
lock-graph blob.

The complete version-set digest should not salt every action indiscriminately.
Doing so would invalidate the world when an unrelated package changes. Analysis
selects the target's reachable dependency closure, and the selected package
trees enter that action's input Merkle root:

```text
closure(target, platform, version_set) -> selected package artifacts
selected package artifacts             -> action input Merkle root
```

Only affected closures receive new action identities. Policy or materializer
changes that can alter output independently of package bytes remain explicit
action inputs.

## Why this is minimal

A tree cannot represent shared dependencies without duplication; a DAG can.
Cycles have no ordinary bottom-up build value, so rejecting them removes the
need for fixed-point semantics. A Merkle tree is the recursively compositional
identity of a directory: equal root digests prove equal canonical descendants,
subject to the hash assumption. A hash map is the minimal expected-constant-time
memo table from a complete action identity to its prior result.

For `V` demanded actions and `E` dependency edges, graph evaluation is
`O(V + E)`. One file edit changes `O(path depth)` directory nodes, and equal
subtree digests prune comparison and transfer. Cache lookup is expected `O(1)`,
while CAS storage and network traffic are proportional to new content rather
than workspace size.

This model maximizes reuse by encoding semantic equivalence rather than build
history. Nix reaches the same conclusion with content-addressed outputs and
quotient hashing: operational changes that provably preserve content should not
force transitive rebuilds. The Remote Execution API likewise identifies an
action by its command and input-root digests and stores results by that action
digest.

## Secure reuse

[Cursor's codebase-indexing design](https://cursor.com/blog/secure-codebase-indexing)
demonstrates two further consequences of the same primitive. Equal subtree
digests let near-identical workspaces reuse unchanged work, and possession of
content can be proven before results from a shared index are revealed. Bessemer
can later use Merkle inclusion proofs or equivalent CAS authorization for
tenant-safe cache sharing.

Cursor's similarity hash is useful only for finding a reuse candidate. It is
approximate and therefore must never authorize a Bessemer cache hit. Build
correctness continues to require exact cryptographic identities and verified
declared inputs.

## References

- [Bazel Remote Execution API](https://github.com/bazelbuild/remote-apis)
- [upstream architectural model](https://oss.dedaluslabs.ai/bsmr/concepts/architecture/)
- [Nix content-addressed derivation outputs](https://nix.dev/manual/nix/stable/store/derivation/outputs/content-address.html)
- [Securely indexing large codebases](https://cursor.com/blog/secure-codebase-indexing)

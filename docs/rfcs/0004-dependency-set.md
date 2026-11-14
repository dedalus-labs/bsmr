---
rfc: "0004"
title: "DependencySet"
authors: ["@windsornguyen"]
state: discussion
discussion: https://github.com/dedalus-labs/bsmr/pull/152
labels: ["dependency-set", "compatibility", "remote-cache", "deployment"]
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines cross-repository dependency rules, exact locks, and release proofs. -->

# DependencySet

## Summary

Bessemer should define a DependencySet: the authoritative graph of components,
provided and required interfaces, compatibility ranges, target platforms,
patches, and provenance rules for a build. An exact `bsmr.lock` records one
resolution satisfying those rules. A separate receipt records the build and
test evidence for that exact resolution.

Ordinary builds consume the lock without changing it or using the network.
Lock updates are explicit. Deployment systems consume a certified receipt and
never resolve versions themselves.

This generalizes Amazon's version-set idea beyond package versions without a
central package monolith. Bessemer owns deterministic rule evaluation,
selection, and proof. Native ecosystem resolvers continue to own pnpm, Cargo,
uv, and Go module semantics.

## Context

The Dedalus monorepo, the `dedalus-labs/lynx` kernel, virtual-machine images,
guest runtime,
and hypervisor move independently. Git commits and artifact digests identify
exact bytes, but they do not state whether those bytes work together. A branch
head or semantic version range can select plausible inputs without proving a
compatible system.

Bessemer already has the lower-level primitives:

- [`VersionSet`](https://github.com/dedalus-labs/bsmr/blob/main/app/bsmr_common/src/version_set.rs)
  domain-separates and hashes a canonical graph root;
- DICE, Bessemer's incremental computation graph, invalidates only affected
  graph nodes;
- action keys and Merkle input trees, whose parent hashes authenticate their
  children, preserve exact build inputs;
- the local and Bazel Remote Execution API caches store action results by
  digest; and
- native ecosystem adapters import committed lockfiles without inventing a
  second package-manager interface.

The existing `VersionSet` type is a provisional CAS seed for a resolved graph.
It accepts opaque canonical bytes and has no external consumers. The
implementation should replace it with separate `DependencySet` and
`DependencyLock` identities before either format becomes public.

Dedalus already has two adjacent contracts to reuse:

- the Dedalus Compute Service (DCS) artifact manifest binds the guest contract,
  kernel digest, root filesystem, capabilities, and snapshot topology; and
- Furl stores an immutable release receipt reference containing an Amazon
  Simple Storage Service (S3) object version and SHA-256 digest, a 256-bit
  cryptographic content hash, then deploys an already-built Open Container
  Initiative (OCI) image digest.

The missing layer is a shared DependencySet contract, exact dependency lock,
and verifier. DCS and Furl should consume those results instead of maintaining
separate compatibility rules.

## Goals and non-goals

### Goals

- Lock exact Git revisions, canonical source trees, patches, and artifacts.
- Express compatibility through named ordered contracts and ranges.
- Resolve the newest compatible candidate deterministically when explicitly
  updating a lock.
- Verify an existing lock quickly and without network access.
- Invalidate only build closures affected by a changed component.
- Produce an immutable receipt that Furl and other deployment systems can
  verify before rollout.
- Use the Bazel Remote Execution API as the cache boundary so Blacksmith and
  other conforming services require no Bessemer-specific storage protocol.
- Explain why a candidate was selected or rejected.

### Non-goals

- Treating Git hashes as ordered versions.
- Replacing pnpm, Cargo, uv, or Go module resolution.
- Updating a lock during `bsmr build`.
- Letting deployment infrastructure resolve or repair a DependencySet.
- Salting every action with the complete DependencySet or lock digest.
- Supporting multiple incompatible implementations through fallback behavior.
- Deleting the retained Android surface while its RFC pull request remains
  open.

## Determination

### Separate rules, resolution, and certification

| Layer | Question | Authority |
| --- | --- | --- |
| Rules | What may Bessemer select? | DependencySet component manifests |
| Resolution | Which exact bytes satisfy those rules? | `bsmr.lock` |
| Certification | Did this resolution pass required evidence? | Immutable release receipt |

A full Git commit hash is an exact source locator. A SHA-256 tree or artifact
digest is the content identity admitted to Bessemer's content-addressed store
(CAS). Neither value is a compatibility version. Git ancestry can prove that
one commit contains another, but it cannot promise that an interface remained
compatible between them.

Each component therefore publishes one integer for each interface it provides.
The integer increases only when the interface contract changes. Consumers
require half-open ranges, with an inclusive lower bound and exclusive upper
bound, such as `>=7,<8`. A breaking change normally increments the contract
once; repository commits may advance many times without changing it.

Examples of contracts include:

- `dedalus.lynx.uapi`;
- `dedalus.dedalusfs.abi`;
- `dedalus.guest-runtime.protocol`;
- `dedalus.dhv.snapshot`; and
- `dedalus.furl.receipt`.

Contract names are globally stable identifiers. Renaming a contract creates a
new contract. Reusing a version number for different semantics is invalid.

### DependencySet manifests

Each independently released repository or artifact family owns one
`bsmr.component.toml` manifest. The rooted graph of these manifests is the
DependencySet. Each manifest declares the component name, contracts it provides
and requires, target constraints, allowed patches, provenance policy, and where
update candidates may be discovered. It does not pin the selected revision.

```toml
schema = 1
name = "dedalus"

[provides]
"dedalus.host-agent.runtime" = 12

[requires]
"dedalus.lynx.uapi" = ">=7,<8"
"dedalus.dedalusfs.abi" = ">=4,<5"
"dedalus.guest-runtime.protocol" = ">=3,<4"
```

Unknown schema versions, fields, range syntax, or contract names fail before
candidate selection or build execution.

### Dependency lock

The repository root commits one generated `bsmr.lock`. The first format is the
human-readable TOML configuration format. Bessemer parses the manifests into a
sorted `bsmr.dependency-set.v1` rule graph and the lock into a separate
`bsmr.dependency-lock.v1` resolved graph. The existing `bsmr.version-set.v1`
prefix is provisional and has no external consumers. Text ordering and comments
do not affect identity. Semantic changes do.

```toml
schema = 1

[[components]]
name = "dedalus"
repository = "https://github.com/dedalus-labs/dedalus"
revision = "68be40674b8ed3efe892ffd4b0e8a145819e9217"
tree = "sha256:..."
manifest = "sha256:..."

[[components]]
name = "lynx"
repository = "https://github.com/dedalus-labs/lynx"
revision = "9deb34a9f46efd3742c34350485aca488c2b11e0"
tree = "sha256:..."
manifest = "sha256:..."
patches = []

[components.provides]
"dedalus.lynx.uapi" = 7
"dedalus.dedalusfs.abi" = 4

[[components.artifacts]]
name = "vmlinux"
digest = "sha256:..."
```

Every component entry requires:

- one canonical component name;
- one repository URL and immutable full commit hash;
- one SHA-256 digest of the canonical source tree;
- one SHA-256 digest of the source component manifest;
- zero or more ordered, SHA-256-addressed patch blobs;
- the exact provided contract versions selected from that manifest; and
- every built artifact name and digest required by downstream consumers.

The source tree digest prevents a Git implementation detail from becoming the
CAS identity. The repository and commit preserve provenance and review links.
Patch order is semantic and enters the component and dependency-lock identity.

### Resolution and verification

The initial resolver has two operations:

```text
verify(dependency_set, lock) -> compatible exact resolution | conflict
update(dependency_set, lock, selected components, candidate catalog) -> new lock | conflict
```

`verify` is the normal build path. It parses the lock and manifests, validates
all immutable identities, intersects every requirement for each named contract,
and checks that the selected provider version is inside the result. It performs
no candidate discovery and no network access.

`update` is explicit. For each selected component, it reads an immutable or
authenticated catalog of available revisions, orders candidates by their
declared release version, and selects the newest candidate satisfying the
complete requirement intersection. A Git commit timestamp or lexical hash
order never ranks candidates.

Bessemer should use Astral's `astral-version-ranges` interval representation for
range union, intersection, membership, and batched membership. The crate stores
the common single interval inline and performs intersection with a linear merge
over sorted intervals. Bessemer already carries version `0.2.2` transitively
through its pinned uv dependencies.

The first implementation does not need PubGrub, Astral's dependency version
solver. The lock chooses exact components, and verification is range
intersection. Add `astral-pubgrub` only when candidate selection has a real
dependency graph that requires backtracking and conflict derivation. Until then,
a full solver is speculative machinery.

### Commands and lock modes

```console
bsmr deps verify
bsmr deps explain
bsmr deps update lynx
bsmr build apps/api
```

`bsmr build` always behaves like a lockfile `error` mode: missing, stale, or
incompatible state fails without changing the lock. `bsmr deps update` is the
only normal command that selects a different component revision. It prints the
selected and rejected candidates plus the contract that decided each rejection.

The command-line interface should not expose a permissive lockfile-off mode.
Tests that need an isolated fixture provide an explicit fixture lock.

### Patches and rapid iteration

Local source edits remain normal action inputs. DICE and content-hash trees
invalidate only actions reachable from the edited source. The DependencySet
rules do not change when a developer edits the current component. The lock pins
external components; certification binds the current component's exact commit
and tree digest.

An external component patch has two states:

1. a local checkout overlay, usable for development but not certification; or
2. an ordered patch blob recorded by digest in `bsmr.lock`, usable for shared
   cache entries and certification.

A release receipt rejects an uncommitted overlay. This preserves rapid local
iteration without letting an unreviewed working tree impersonate a deployable
dependency resolution.

The DependencySet and lock digests are metadata and policy identities. They must
not be added to every action key. Each action receives only the selected source,
artifact, toolchain, platform, and policy nodes in its reachable closure. An
unrelated Lynx update must not invalidate a web package that does not depend on
Lynx.

### Release receipt

A successful certification job writes one canonical receipt containing:

- receipt schema, DependencySet digest, and dependency-lock digest;
- the exact canonical lock object digest;
- component revisions, tree digests, manifest digests, and patches;
- produced artifact digests;
- required build and test action digests with results;
- builder identity, workflow, run attempt, and source commit; and
- provenance or attestation references.

The receipt is stored as an immutable object. Furl's existing
`ReleaseReceiptReference` already identifies an S3 URI, object version, and
SHA-256 digest. Furl should verify a supported receipt schema and policy before
accepting rollout intent, persist the reference, and deploy only the OCI digest
named by the receipt. Furl never runs Bessemer's resolver.

Production promotion copies the exact Staging-tested artifact digest. It does
not rebuild the dependency resolution.

### Remote cache and Blacksmith

Bessemer already speaks the Bazel Remote Execution API (REAPI) for action-cache
and content-addressed storage operations and uses SHA-256 by default.
Blacksmith's cache implements REAPI through its `bazel-remote` fork. Integration
should therefore be configuration plus conformance, not a Bazel compatibility
shim.

The supported integration needs:

- endpoint and credential discovery suitable for GitHub Actions;
- one explicit instance name or tenant namespace;
- Capabilities, CAS, ByteStream, and Action Cache conformance tests;
- read-only cache access for untrusted pull requests;
- write access only for trusted builds; and
- cache provenance that binds the exact execution platform and toolchain.

A future `useblacksmith/setup-bsmr` action may generate `.bsmr` client
configuration. Bessemer should not pretend that Blacksmith's Bazel-specific
disk and repository cache action configures a non-Bazel client.

### Rust incremental layout experiment

Cargo's new build-directory layout separates final artifacts from internal
build units and is intended to unlock garbage collection and cross-workspace
caching. It remains unstable behind `-Zbuild-dir-new-layout` in current nightly
Cargo.

Bessemer should benchmark it behind an explicit experimental Rust toolchain
mode. The adapter must consume Cargo JSON messages instead of depending on the
internal directory layout. Adoption requires equivalent output tests, private
implementation and public API edit benchmarks, cross-worktree reuse, and
deleted-output restoration. No default changes until those gates pass.

### Native JavaScript build default

The TypeScript frontend currently treats `tsdown.config.ts` as the signal for a
buildable package. The general default should instead execute the package's
exact `scripts.build` command through the locked pnpm runtime. This supports
Vite, tsdown, tsc, tsup, esbuild, and other package-local tools without parsing
their shell syntax inside Bessemer.

The first safe convention is:

- `scripts.build` opts a package into native build inference;
- `dist/` is the default declared output;
- `bsmr.outputs` in `package.json` explicitly replaces that output list; and
- packages without a build script remain source-only packages.

The action consumes the package source closure, frozen pnpm layout, exact Node
and pnpm tools, script text, declared environment, and output contract. An
undeclared output or network dependency fails. The existing tsdown-specific
rule becomes an advanced explicit rule, not a second automatic path.

### Repository pruning

Pruning follows executable reachability, not string matching on Meta notices.
Inherited notices remain on retained code. Each deletion slice must remove one
coherent surface and prove the release binary, self-host build, supported native
frontends, and relevant integration tests still pass.

Classify the tree into:

| Class | Treatment |
| --- | --- |
| Build kernel | Retain while reachable from the release binary or self-host graph. |
| Native TypeScript, Rust, Go, and Python adapters | Retain and simplify around native manifests. |
| Remote execution, content-addressed storage, DICE, and materialization | Retain as core infrastructure. |
| Android | Retain unchanged while RFC PR #19 remains open. |
| Unsupported language rules, examples, and public docs | Delete in independent negative-diff slices. |
| Meta-internal configuration, telemetry, and operational integrations | Delete once no retained target depends on them. |

The public documentation remains an allowlisted product surface. Retained
upstream engineering documents may stay excluded until a deletion slice proves
they have no code-maintenance value.

## Alternatives

| Alternative | Rejected because | Retained idea |
| --- | --- | --- |
| Exact tuples without ranges | Every compatible Lynx build would require a monorepo edit. | Keep exact selections in the lock. |
| Repository semantic-version ranges | One repository exposes several independently changing interfaces, while Git hashes have no semantic ordering. | Use semantic versions for releases where useful. |
| Full PubGrub resolution now | Exact-lock verification needs intersection, not speculative backtracking. | Add PubGrub when a real candidate graph requires it. |
| Bazel's module system as the manifest | Bazel syntax should not become authoritative for a non-Bazel product. | Reuse explicit lock modes, immutable hashes, patches, and reproducible extension state. |
| Resolution during Furl rollout | Deployment could select a tuple that Staging never tested. | Furl verifies a certified receipt and deploys exact digests. |

## Consequences

Users gain one reviewable lock diff and one command to explain incompatibility.
Repositories may move independently while their contracts remain stable.
Breaking interfaces require an intentional contract increment and coordinated
lock update.

The system adds manifest discipline. Contract versions must be maintained and
candidate catalogs must be immutable or authenticated. Incorrectly leaving a
contract version unchanged is a correctness bug; certification tests remain the
backstop.

Cache sharing improves because compatible component changes invalidate only
reachable closures. Storage cost increases for retained DependencySets, exact
locks, patch blobs, receipts, and provenance. Content addressing
deduplicates those objects.

## Validation and rollout

| Phase | Deliverable | Required proof |
| --- | --- | --- |
| 0: contract kernel | Typed DependencySet rules, component identities, contract ranges, and in-memory verification | Exact match, compatible range, missing or duplicate provider, empty intersection, and unrelated component tests |
| 1: lockfile | Strict `bsmr.component.toml`, canonical `bsmr.lock`, `deps verify`, and `deps explain` | Equivalent rules and locks share their respective digests; stale or incompatible locks fail before analysis |
| 2: Dedalus tuple | Lynx, host-agent, guest-runtime, hypervisor, and compute-manifest contracts | One known-good tuple passes DCS suites; an incompatible increment blocks image publication |
| 3: updates and patches | Authenticated candidate catalogs, explicit update command, and content-addressed patches | Deterministic selection explanations; local overlays cannot certify |
| 4: cache and deployment | Blacksmith conformance, Staging receipt, and Furl verification | Production deploys and rolls back exact certified digests without rebuilding |
| 5: ecosystem and pruning | General JavaScript build scripts, Cargo layout experiment, and subsystem deletion slices | Real Vite fixture, matched Rust benchmark, supported builds, retained Android, and synchronized public docs |

The RFC becomes `implemented` only when Phases 0 through 4 are complete. Phase
5 contains independent follow-up work and does not block the core contract.

## Open questions

| Question | Current answer |
| --- | --- |
| Where do candidate catalogs live? | In an immutable signed object referenced by the root repository. Mutable release listings are discovery only unless captured by digest. |
| Do contracts use integers or semantic versions? | Integers. A contract is an interface epoch, not a product release. Another ordered type requires a new schema. |
| Does the lock include test policy? | No. The lock identifies inputs and compatibility; a separately versioned certification policy identifies required evidence. |
| How is a repository tree identified? | Bessemer hashes a canonical imported tree into its CAS. The Git object remains provenance, not build identity. |

## References

- [Bessemer hermetic build core](../concepts/hermetic_build_core.md)
- [RFC 0001 pnpm lockfile discussion](https://github.com/dedalus-labs/bsmr/discussions/12)
- [Astral version ranges](https://github.com/astral-sh/pubgrub/tree/363fea1e03820a02a56dc888846922c445227620/version-ranges)
- [Astral PubGrub](https://github.com/astral-sh/pubgrub/tree/363fea1e03820a02a56dc888846922c445227620)
- [Bazel module resolution](https://bazel.build/external/module)
- [Bazel lockfile modes](https://bazel.build/external/lockfile)
- [Bazel Remote Execution API](https://github.com/bazelbuild/remote-apis)
- [Blacksmith `bazel-remote` fork](https://github.com/useblacksmith/bazel-remote)
- [Blacksmith Bazel setup action](https://github.com/useblacksmith/setup-bazel)
- [Cargo build cache](https://doc.rust-lang.org/cargo/reference/build-cache.html)
- [Cargo build-directory layout v2](https://blog.rust-lang.org/2026/03/13/call-for-testing-build-dir-layout-v2/)
- [Oxide RFD lifecycle](https://rfd.shared.oxide.computer/rfd/0001)

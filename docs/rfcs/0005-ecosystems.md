---
rfc: "0005"
title: "Native ecosystem contracts"
authors: ["@windsornguyen"]
state: ideation
discussion: null
labels: ["ecosystems", "interfaces", "builds"]
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Proposes the common contract for native package adapters and custom rules. -->

# Native ecosystem contracts

## Summary

Popular projects should keep their native manifests, tool configuration and
build commands. BSMR adapters translate those declarations into typed targets,
actions and artifacts using the existing engine. Built-in adapters and custom
rules follow the same execution and output contracts.

## Context

A package can compile its CLI and then build web assets. Completing the compiler
step does not complete the package. Treating a recognized tool configuration as
the whole build can produce a successful target with missing runtime files.

BSMR already translates native manifests into Starlark and shares its action
engine with explicit build files. The missing contract is how an adapter selects
a complete target and communicates what its consumers receive.

## Goals and non-goals

Support conventional projects without authored build rules. Make unusual builds
explicit and composable. Preserve native tool semantics and expose precise
errors for unsupported contracts. Keep scheduling, caching, resource ownership
and cancellation in the existing engine.

This proposal does not introduce a dependency solver, plugin registry, dynamic
Rust ABI, workflow interpreter or replacement rule language. Existing rules
remain available. Removing inherited subsystems requires a separate proposal.

## Determination

### Native commands define package intent

Use an explicitly selected native build task as the package's build contract.
For JavaScript, a declared package script can execute as one action through the
pinned package manager. Do not parse its shell text into a second build graph.
Record the shell and other required tools in the execution environment.

Infer targets only from supported, unambiguous conventions. A repository can
explicitly select a target and declare missing output information. Unsupported
toolchains, ambiguous target selection and unbounded input contracts fail before
execution. A compiler-only target must be named as such.

Keep tool-specific adapters when they provide useful target discovery, typed
outputs or independently cached work. Split a package task only after real
measurements justify it and artifact equivalence is established.

### Checked values cross the adapter boundary

These are proposed semantic contracts, not new public Rust declarations.
Reuse existing artifact, path, provider and execution types where they fit.

| Value | Required invariant |
| --- | --- |
| Package identity | A validated package root and authoritative native manifest identify one package. |
| Target identity | A package owns the target. Selection identifies one declared operation and rejects ambiguity. |
| Toolchain | Exact verified artifacts and execution platform satisfy the target's tool requirements. |
| Input set | Tracked source, configuration, dependency artifacts and relevant environment identify the work. |
| Action plan | Typed arguments reference declared artifacts. Outputs have one writer. Dependencies form a valid graph. |
| Output contract | Required files, directories, entrypoints and runtime resources have explicit ownership and placement. |

Use checked constructors and explicit variants for invalid-state boundaries.
Do not pass an unvalidated options map between adapters and the engine. Preserve
ecosystem-specific provider types where a library, executable or package needs
different information. Arbitrary paths are not substitutes for artifact handles.
Keep package paths distinct from optional registry names. A manifest selected
for dependency installation need not declare a publishable package or build target.

Adapters plan work through tracked reads. Tool-based discovery and acquisition
must also be tracked work. Compilers and build backends execute through the
engine, including scripts invoked by the package manager.

### The engine owns execution policy

Adapters declare requirements. The engine validates platform and isolation
compatibility, computes action identity, and determines cache eligibility.
An adapter cannot grant itself unrestricted network access or trusted cache
writes. Code that affects execution must be represented in the action's inputs
or semantics. Unrelated adapter changes must not invalidate every target.

Acquisition authenticates dependency bytes before build execution. Build actions
consume those artifacts under their declared access policy. Native resolvers
retain ownership of dependency semantics. A lockfile alone does not establish
isolation or deterministic output.

### Consumers receive complete artifacts

A target succeeds only after its required outputs are validated. An executable
target includes its runtime file closure and invocation contract. Composition
passes artifact references and rejects conflicting output ownership.

For the CLI-plus-web case, the package build must supply both the executable and
the assets at the paths its renderer uses. A later refinement can cache those
producers separately while retaining the same package output contract.

## Alternatives

Requiring Starlark for ordinary projects duplicates native configuration and
increases adoption cost. A separate plugin engine duplicates execution policy.
Hardcoding one mode for every popular tool creates coupled discovery and runner
switches. Keep native conventions at the boundary and reusable rules underneath.

## Consequences

Whole-package actions provide broad compatibility with coarser cache reuse.
Finer actions are an optimization with an explicit equivalence obligation.
Projects must declare inputs or outputs that native conventions cannot identify.
Changing a default target's meaning requires a documented cutover.

## Validation and rollout

Use real upstream projects at immutable revisions. These manifests were inspected.
The corpus has not yet passed BSMR qualification. An unchanged Vite checkout
fails BSMR 0.0.5 intake because a selected test-fixture manifest containing only
`type: module` has no package name. This identifies a package-model compatibility
gap before any compiler runs. `pnpm list -r --depth -1 --json` accepts the same
fixture as an unnamed workspace member. No upstream manifest was changed.

| Project | Contract to exercise |
| --- | --- |
| [Vite](https://github.com/vitejs/vite/tree/e9078f865cdff6bed77cd729214a7e2868f126b5) | Workspace package build uses Rolldown, a separate declaration build and typechecking. The inspected root pins pnpm 12.4.2. |
| [tsdown](https://github.com/rolldown/tsdown/tree/eb40c95efdf7f98d0aa7b0178a3b7322986bd419) | Self-hosted Node entrypoint with conditional exports and packaged declaration files. |
| [Flask](https://github.com/pallets/flask/tree/d73fa1cdcbd8b1465c151db8924ba58b1dd14e35) | Native Python build backend, wheel contents and installed console script. |
| [ripgrep](https://github.com/BurntSushi/ripgrep/tree/3fce3b5bb0236da2df6d99672afb8a719642eca7) | Cargo workspace, build script, executable and optional native dependency. |

Record any toolchain selection or lock acquisition in the qualification recipe.
Do not rewrite upstream build commands or versions to make a test pass. Report
unsupported contracts explicitly. Begin with one JavaScript package and one
different ecosystem before stabilizing an extension API.

For each case, compare the native build with BSMR from clean outputs. Run the
result, restore it in another checkout, mutate an output, and change a source,
dependency, toolchain and unrelated file independently. Check cancellation,
missing outputs, output conflicts and prohibited access. Measure cold, warm and
restored builds separately. Cache hits must restore every required runtime file.

## Prototype

The `pnpm_task` rule implements the first
explicit adapter. It consumes existing source and install artifacts, runs a
package script and checks declared output types before publication. It supports
local POSIX execution with host utilities and disables cache uploads.
Native discovery and a stable external adapter API remain proposals.

## Open questions

Where should an explicit output declaration live? Prefer existing native
metadata when it is sufficient. Prototype one small BSMR-owned declaration only
for facts the ecosystem does not express. Test that choice against the corpus
before selecting a format.

How should external programs author adapters? Keep first-party adapters and the
existing Starlark boundary initially. Stabilize a versioned external contract
only after multiple consumers establish its required operations.

## References

- [Native lowering](https://github.com/dedalus-labs/bsmr/blob/main/app/bsmr_interpreter_for_build/src/interpreter/dice_calculation_delegate.rs)
- [Execution requests](https://github.com/dedalus-labs/bsmr/blob/main/app/bsmr_execute/src/execute/request.rs)
- [Artifact ownership](https://github.com/dedalus-labs/bsmr/blob/main/app/bsmr_artifact/src/artifact/build_artifact.rs)
- [pnpm dependency contract](https://github.com/dedalus-labs/bsmr/discussions/12)

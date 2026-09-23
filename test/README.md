<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Documents the native TypeScript cache and action-runtime qualification. -->

# TypeScript cache checks

Run `node test/typescript/cache.ts /path/to/bsmr` from the repository root.
CI runs the same command through Hollywood's `typescript/cache` action, defined
in `ci/typescript/cache.ts`. `localActionPath` sets the nested generated route.
Use short operation names under a subject directory for new local actions.

To test rule edits with an installed binary, pass the source prelude as the
second argument: `node test/typescript/cache.ts /path/to/bsmr prelude`.
The harness copies it into its temporary workspace and selects it through
`.bsmr.local`. Omit that argument to test the binary's bundled rules.

The fixture compiles with a frozen pnpm install, restores deleted outputs,
checks cached bytes survive output mutation, and rebuilds changed source.
It also runs a bundled Hollywood action from a detached directory to check
relative file inputs, GitHub output records and missing-input failure.

The recorded hashes identify entrypoint files. This fixture tests the action
runtime and cache behavior. GitHub worker lifecycle and generated action
metadata require connected runner qualification.

## Package tasks

Run `node test/pnpm/task.ts /path/to/bsmr` to test the binary's embedded `pnpm_task`
rule with a real frozen install and compiler. Append `prelude` to test this
checkout's rules explicitly. Each phase reports its elapsed time. It checks detached code and assets,
individual output selection, warm reuse, clean rebuilds and missing-output errors.
See the [task contract](../prelude/toolchains/pnpm/README.md).

## Rust CI caches

The self-host qualification job builds the engine with its pinned nightly
compiler and the Cargo planner with stable Rust. Each compiler has a separate
target directory and cache key. The planner cache owns `tools/cargo/target`.
The job copies the built planner beside the engine before running tests.

Successful pushes to `main` save these caches. Pull requests and merge groups
only restore them. A missing planner cache requires a cold compile. Cargo still
checks the restored dependency outputs before reusing them.

## Rust dependencies

Run `node test/rust/dependencies.ts /path/to/bsmr` with the matching
`bsmr-cargo` beside the binary and Rust 1.97.1 installed. It builds and runs an
executable using a locked registry crate and a nested Git package, then verifies
that a warm build runs no compiler actions and leaves the lockfile unchanged.
Pass `1.98.0` as the third argument to qualify that installed toolchain instead.

`python3 -B -m unittest discover -s prelude/git/tools/tests -p '*_test.py'` checks
that ambient Git filters and checkout hooks cannot change pinned sources.

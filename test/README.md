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

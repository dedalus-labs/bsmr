<!-- ===----------------------------------------------------------------------=== -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# Getting started

Use Bessemer with an existing pnpm, Cargo, or Go workspace.

1. [Install Bessemer](install.md).
2. Run `bsmr init` at the workspace root.
3. Run `bsmr build <path>` for one package.

The package path is a normal repository-relative directory such as `apps/api`
or `packages/rust/dfa`. Bessemer reads the native manifest and includes the
package's dependencies.

Continue with the [Quick Start](quickstart.md).

<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines the pnpm install and package-task boundary. -->

# pnpm tasks

A package's build script can compile code and generate assets. `pnpm_task` runs
that script through the pinned package manager and publishes the outputs you
declare. The package keeps its command. BSMR owns scheduling and artifacts.

```text
native manifests -> frozen install ----+
declared source artifact --------------+-> package script -> checked outputs
```

For a native workspace package at `probe`, put this in `tasks/BUILD.bsmr`:

```python
load("@prelude//toolchains/pnpm:defs.bzl", "pnpm_task")

pnpm_task(
    name = "build",
    install = "root//:__bsmr_dependencies",
    source = "root//probe:__bsmr_sources",
    package_root = "probe",
    script = "build",
    outputs = {"dist": "directory", "web/index.html": "file"},
)
```

Run `bsmr build tasks:build`. A consumer can select
`tasks:build[web/index.html]` as an ordinary artifact. Keep this explicit build
file outside native package directories so it does not replace their discovery.
The generated source/install labels are a prototype integration surface.

| Input | Contract |
| --- | --- |
| `install` | `PnpmInstallInfo` from a successful frozen install, including its exact Node and pnpm artifacts. |
| `source` | Directory artifact containing the complete workspace-relative source and configuration closure. |
| `package_root` | Normalized workspace-relative directory, or `.`. |
| `script` | One declared package script. pnpm interprets its shell command and script chaining. |
| `outputs` | Non-overlapping package-relative paths with required `file` or `directory` types. |

The task copies sources into scratch and links the frozen dependencies. Source
manifests must match the install. pnpm's automatic pre-script installation is
disabled because acquisition already belongs to `pnpm_install`.
Workspace executable links point into the declared source copy. External package
executables keep their frozen install targets, preserving pnpm's bin selection.
Required outputs must be new paths. Missing outputs, wrong types, failed scripts
and output symlinks fail the action. All outputs are checked before publication.
Their paths remain relative to the package, including nested assets.

This prototype supports POSIX hosts. It uses `/bin/sh` and host utilities from
`/usr/bin:/bin`, with pinned Node and pnpm first on `PATH`. It does not sandbox
scripts or restrict network access. Cache uploads are disabled. The local graph
can reuse an unchanged result, but a clean rebuild executes the task again.
Scripts must not mutate dependency artifacts. Consumers still declare runtime
dependencies and package metadata when their outputs require them.

Validate the runner with `pnpm run ci test`. Run the real compiler/artifact test
with `node test/pnpm/task.ts /path/to/bsmr`. It uses the binary's embedded prelude.
Pass `prelude` as the final argument to test a source-prelude override explicitly.
The harness reports each phase in milliseconds and native critical-path entries
in microseconds.
See [pnpm's script contract](https://pnpm.io/cli/run) and the
[native workspace guide](../../../docs/users/languages/typescript/pnpm.md).

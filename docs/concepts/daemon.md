---
id: daemon
title: Daemon (buckd)
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->


import { FbInternalOnly } from 'docusaurus-plugin-internaldocs-fb/internal';

The first time that a Bessemer command is run, Bessemer starts a daemon process for
the current project. For subsequent commands, Bessemer checks for the running
daemon process and, if found, uses the daemon to execute the command. Using the
Bessemer daemon can save significant time as it enables Buck to share cache between
Bessemer invocations.

By default, there is 1 daemon per [project](./glossary.md#project) root, you can
run multiple daemons in the same project by specifying an
[isolation dir](./glossary.md#isolation-dir).

While it runs, the Buck daemon process monitors the project's file system for
changes. The Buck daemon excludes from monitoring any subtrees of the project
file system that are specified in the `[project].ignore` setting of
`.bsmrconfig`.

You can see detailed information about the status of the daemon by running
`bsmr status`.

## Killing or disabling the Buck daemon

The Buck daemon process is killed if `bsmr clean` or `bsmr kill` commands are
run. Note that they won't kill the daemon associated with custom isolation dirs.
To do that, run using the `--isolation-dir` option
(`bsmr --isolation-dir <dir> <command>`)

<FbInternalOnly>

The Daemon is also killed when:

- The `bsmr killall` command is run.
- A new bsmr version is available.

</FbInternalOnly>

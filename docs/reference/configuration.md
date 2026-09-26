<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# Projects and configuration

`bsmr init` creates one `.bsmr` file at the project root. It serves two roles:

1. `[project] root = .` marks the root.
2. Other sections override advanced defaults.

Most native TypeScript and Rust projects should keep the generated file
unchanged. Their ecosystem manifests remain authoritative for packages,
dependencies, and tool versions.

## Minimal project marker

```ini
[project]
root = .
```

A nested `.bsmr` without this marker may add cell-local settings. It does not
create a second project.

## When to edit it

Edit `.bsmr` when you need one of these advanced features:

- remote execution or a remote cache;
- a custom local sandbox bundle;
- a custom execution platform;
- an additional cell;
- a custom Starlark prelude or toolchain; or
- a repository-wide parser or output policy.

Do not copy configuration from another repository without understanding it.
Configuration participates in action keys and can invalidate cached work.

Run `bsmr --help` to discover advanced commands. Their exact flags and defaults
are recorded in the [command-line reference](cli.md).

The `[sandbox]` section and `--sandbox` workflow are documented in
[Sandboxed builds](../users/sandboxing.md).

## Watch source changes on Linux

The native watcher discards its incremental dependency graph when the operating
system reports lost events. This also applies when the notification has no file
paths or only names ignored outputs. The next build rereads source state before
reusing results.

For projects with large build-output trees, use the existing Watchman backend.
Watchman can exclude `bsmr-out` before walking its directories or registering
file watches. Install [Watchman](https://facebook.github.io/watchman/docs/install)
and make its CLI available on `PATH`.

Set the backend in the project's `.bsmr` or `.bsmr.local`:

```ini
[bsmr]
file_watcher = watchman
```

Add `bsmr-out` to `ignore_dirs` in the project's `.watchmanconfig`. Preserve
any other settings or exclusions in that file.

```json
{"ignore_dirs": ["bsmr-out"]}
```

After any builds finish, run `bsmr kill` in the project. The next build starts
a daemon with the selected backend. Daemon startup reads configuration files,
so a one-off `-c bsmr.file_watcher=watchman` does not select this backend.
A failed Watchman connection fails the build. Do not exclude source directories
whose changes must invalidate build results.

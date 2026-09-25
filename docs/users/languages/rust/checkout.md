---
id: checkout
title: Build-script checkout inputs
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Explains source and Git ownership for native build scripts. -->

# Build-script checkout inputs

A build script that embeds a Git revision must observe the checkout used by its
build. Giving it only its package directory loses the repository root, staged
changes and files in sibling packages. Reusing that script's output can then
embed an old revision or incorrectly mark a clean checkout as dirty.

For first-party scripts running in a verified Linux namespace, BSMR assembles
the workspace's native source packages in their original relative locations.
The view retains hidden files, empty source directories and the literal targets
of relative symlinks. Empty submodule directories matter because Git reports a
deleted directory as a dirty checkout even before the submodule is initialized.
Ordinary `copied_dir` calls still rebase links to retain their original referent.
Source reconstruction explicitly uses `symlinks = "preserve"` instead.

At command start, BSMR hashes the Git inputs that read-only identity queries use:
HEAD, the real staging index, shared indexes, refs, packed refs, shallow history,
object stores and repository-local exclude rules. Linked worktrees contribute
their own HEAD and index plus the common repository's refs and objects. Changes
invalidate the persistent engine's view even when no Rust file changed.
The view retains the required empty `refs/` directory when every reference is packed.

Git inputs are copied through checksum-verified local downloads. A file changed
after capture cannot be substituted under the earlier checksum. Actions get a
minimal Git configuration. Host credentials, includes and executable hooks are
not copied. The runtime must supply Git itself.

The existing build-script runner copies this declared view into its private
output, runs from the selected package directory and publishes the package and
generated files through the existing native Rust rules. The compiler also keeps
that workspace as a declared input, so relative includes can reach sibling data
files. Git queries use the same working files that the script can inspect or modify.

| Boundary | Owner |
| --- | --- |
| Current Git metadata and its checksum | Command setup |
| Source names and bytes | Native package inputs |
| Workspace assembly and relative links | Artifact copy actions |
| Private writable files and Cargo directives | Build-script runner |

This is conservative input tracking. A change elsewhere in the checkout can
rerun a first-party script. Hashing object packs and copying the workspace also
have a cost that must be included in rebuild measurements. Concurrent edits are
not an atomic snapshot contract. Non-native source package boundaries are
rejected when a script needs this view rather than silently omitting files.

Run `python3 test/rust/identity.py /path/to/bsmr /path/to/runtime.json` to compare
script output with Git across clean, untracked, staged, commit-only and linked
worktree changes. `python3 test/rust/snapshot.py /path/to/bsmr` separately checks
relative symlinks after two stages of native artifact copying.

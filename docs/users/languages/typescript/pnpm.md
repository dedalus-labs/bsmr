---
id: pnpm
title: pnpm
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Documents RFC 0001's pinned pnpm adapter and its hermeticity boundary. -->

# pnpm

BSMR can run one pinned pnpm install as a cached build action. This is the
milestone-zero adapter from [RFC 0001](https://github.com/dedalus-labs/bsmr/discussions/12):
pnpm interprets `package.json` and
`pnpm-lock.yaml`, while BSMR owns the exact Node and pnpm artifacts, declared
project inputs, mutable package-manager state, and install output.

The adapter deliberately runs one install for a repository lockfile. Do not
create one installer per workspace package. BSMR should parallelize compilation,
tests, and packaging above the installed dependency graph rather than asking
multiple pnpm processes to contend over equivalent resolution and linking work.
The milestone-zero adapter supports exact pnpm 10 and 11 toolchains. It sets
pnpm 11's `pmOnFail` policy to `error` and invokes `pnpm with current`, so pnpm
cannot silently replace the verified toolchain from the project's
package-manager declaration. pnpm 10 predates that command; the adapter disables
its package-manager version management and invokes the verified CLI directly.

## Toolchain

Download Node and the platform-independent `pnpm` npm package with
`http_archive`. Every archive must have a repository-pinned SHA-256 digest.
The pnpm `package_manager` value must exactly match the standard
`packageManager` declaration in the repository's `package.json`, including its
Corepack SHA-512 digest.

```python
load(
    "@prelude//toolchains/pnpm:defs.bzl",
    "node_distribution",
    "pnpm_distribution",
    "pnpm_toolchain",
)

http_archive(
    name = "node_26_5_1_darwin_arm64",
    sha256 = "f4387df0b46556516d19abf2f2d6806481ac8368aa7f9d96bafed422a56a1d01",
    strip_prefix = "node-v26.5.1-darwin-arm64",
    urls = ["https://nodejs.org/dist/v26.5.1/node-v26.5.1-darwin-arm64.tar.gz"],
)

http_archive(
    name = "node_26_5_1_linux_x64",
    sha256 = "2b07f09c218d473a26442bff5a90151f53f7b7c0a23bad244eda2c26303a2ba7",
    strip_prefix = "node-v26.5.1-linux-x64",
    urls = ["https://nodejs.org/dist/v26.5.1/node-v26.5.1-linux-x64.tar.gz"],
)

http_archive(
    name = "pnpm_11_20_0",
    sha256 = "34e198cb1e43237517ecedfd31f9ae26a6c0a3e5366ce58a2d05f4b21fb5f19a",
    urls = ["https://registry.npmjs.org/pnpm/-/pnpm-11.20.0.tgz"],
)

node_distribution(
    name = "node_distribution",
    root = select({
        "config//os:linux": select({"config//cpu:x86_64": ":node_26_5_1_linux_x64"}),
        "config//os:macos": select({"config//cpu:arm64": ":node_26_5_1_darwin_arm64"}),
    }),
    version = "26.5.1",
)

pnpm_distribution(
    name = "pnpm_distribution",
    package_manager = "pnpm@11.20.0+sha512.9a6f330a95b66446ea088faf1521405a8a01f07fde7124cc9958dfed52d4bb436737e65b08f85f37b46fcba375092558ac51262b816844b22f63406ed166bfee",
    root = ":pnpm_11_20_0",
)

pnpm_toolchain(
    name = "pnpm",
    node = ":node_distribution",
    pnpm = ":pnpm_distribution",
    visibility = ["PUBLIC"],
)
```

Add POSIX platforms by pinning their official Node archives and extending the
`select`. The frozen install adapter fails closed on Windows until BSMR can
produce relocatable executable links there. The example tracks BSMR's current
default, pnpm 11.20.0. Consumer
toolchains remain configurable, so a repository such as the Dedalus monorepo
may retain an exact pnpm 10 pin. Every selected version must be supported,
exact, and digest-pinned. There is no system-Node, global-pnpm, or
implicit-latest fallback.

## Frozen install

Declare the root manifest, authoritative lockfile, and every other project file
the install can read. A list uses each artifact's short path; an explicit map is
available when the desired project-relative path differs.

```python
load("@prelude//toolchains/pnpm:defs.bzl", "pnpm_install")

pnpm_install(
    name = "dependencies",
    package_json = "package.json",
    pnpm_lock = "pnpm-lock.yaml",
    srcs = glob(
        ["packages/**", "pnpm-workspace.yaml", ".npmrc"],
        exclude = ["**/node_modules/**"],
    ),
)
```

The action fails before pnpm starts unless all of these invariants hold:

- The executing Node runtime exactly matches `engines.node`.
- The configured pnpm version and SHA-512 digest exactly match `packageManager`.
- `pnpm-lock.yaml` exists and `pnpm install --frozen-lockfile` accepts it.
- BSMR's `.bsmr` state directory is not supplied as a project input.
- The declared output does not already exist.

The runner copies the declared inputs into a writable action output and redirects
pnpm's store, home, Corepack home, npm cache, and user npm configuration into
BSMR-provided action scratch space outside that cached output. Repository `.npmrc`
files remain ordinary declared inputs. Ambient home-level npm configuration and
pnpm's global store are never consulted.

On POSIX, pnpm must emit relative executable symlinks instead of command shims
containing the action's absolute path. After installation, BSMR removes pnpm's
`.modules.yaml` and `.pnpm-workspace-state-v1.json` bookkeeping because those
files contain absolute store paths and wall-clock timestamps. Mutable store,
verification, and metadata-cache contents remain in scratch space; only the
relocatable workspace becomes the action output and enters the CAS.

The adapter disables lifecycle scripts. Package materialization must not run
arbitrary install hooks with ambient access and then publish their results under
a supposedly hermetic action key. Packages that require lifecycle preparation
remain unsupported until those hooks become explicit sandboxed BSMR actions.

## Hermeticity boundary

This adapter gives dependency installation a stable action key and cacheable
output, but it is not the final hermetic JavaScript model. The install action
requires registry network access on a cache miss. Registry behavior therefore
remains outside the declared input tree even though downloaded package contents
are verified against the frozen lockfile.

Later RFC 0001 milestones consume the lockfile graph directly, fetch each
integrity-addressed artifact into BSMR's CAS, materialize the pnpm-compatible
layout without network access, and promote lifecycle scripts into separately
sandboxed and cached BSMR actions. Until those milestones land, describe this
feature as the pinned pnpm adapter, not as a fully hermetic JavaScript build.

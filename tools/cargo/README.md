<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Describes the internal configured-unit planning boundary. -->

# Cargo planning

`bsmr-cargo` reads one JSON request from stdin and writes Cargo's configured
version-5 graph to stdout. Cargo resolves features, profiles, host/target units, and
test dependencies. The helper does not invoke Cargo's compilation runner.

```text
trusted workspace + compiler + request -> Cargo BuildContext -> unit graph
```

The helper holds an exclusive source-home lease through graph output. Git reads
use process-local empty system and global configuration. Executable credential
providers, Git configuration includes, hooks, and unsafe cache paths fail before
compiler probes. The caller must exclude programs that do not honor this lease.

The caller supplies absolute `manifest`, `cargo_home`, `rustc`, and
`target_directory` paths. It owns the files, configuration, and environment for
the duration of the request. Planning requires an existing lockfile. `source_policy` selects frozen offline
resolution or `acquire-locked`, which fetches dependencies needed by the selected
graph without changing the lockfile or fetching every workspace dependency.

The nonempty `packages` array, `mode`, and `target_filter` select the roots.
Cargo resolves the selected packages together, including shared dependency
features. Modes are `build`,
`test`, and `check`. Filters are `package`, `library`, `binary` with a `name`,
and `integration-test` with a `name`. The request also supplies `features`,
`default_features`, `all_features`, `profile`, and a target triple or null.
Unknown request fields and conflicting compiler overrides fail. Compiler flags
are limited to cfg values, lints, and scalar optimization settings. Response
files, compiler extensions, and file-based overrides fail before compiler probes.
Inactive target tables may carry linker arguments or target features, but selected units using them
require a declared native execution contract.

The `targets` filter takes a nonempty `targets` array of package, kind and name
records. Kinds are `library`, `binary` and `integration-test`. Every package must
appear in `packages`. The graph retains these roots in request order and only
their reachable dependencies. Equal target names in separate packages remain
distinct. Cargo still resolves features jointly before this root projection.

The graph can contain multiple roots. This planner contract does not yet make
the native CLI combine its independently selected entrypoints. The caller must
carry the complete selection to this boundary and preserve it during lowering.

Each unit includes its source identity, package environment, declared and active
features, compiler and documentation flags, lints, effective linker, and resolved
dependency aliases. Each unit's `harness` preserves Cargo's choice between
rustc's test runner and a custom `main`. The graph records Cargo's workspace
package boundaries and selected path dependencies, so compiler source views
retain ancestor data without importing unrelated crates.
The response reports the resolver and actual compiler versions.
Registry archives must match their locked checksum. Cached registry manifests
must match the verified archive. Git package and workspace manifests must match
regular-file blobs in the locked commit. Directory source replacements fail.
Each source includes an `artifact`: a workspace package or an archive with its
SHA-256, length, tree prefix and package path. Registry archives use their locked checksum.
Git archives retain the complete locked tree, including executable bits and
symlinks. Package projections keep the tree as an action input, so links and
relative reads can reach sibling files without escaping the declared source.
The planner writes them into its source home and returns a local file URL.
Native archive materialization verifies the checksum before compilation. It
does not fetch Git again or give compiler actions network access. Checkout
filters and export attributes cannot rewrite these bytes. Git submodules are
rejected explicitly. Cargo's mutable checkout is never a compiler input.

The resolver pins Cargo 0.98.0 and admits Rust 1.97.1, 1.98.0, and
nightly-2026-04-11. Its standalone workspace isolates Cargo's native dependencies
from the build engine. See [Cargo's unit graph](https://doc.rust-lang.org/cargo/reference/unstable.html#unit-graph).

```console
cargo +1.98.0 build --manifest-path tools/cargo/Cargo.toml --locked -j 2
python3 tools/cargo/check.py tools/cargo/target/debug/bsmr-cargo
python3 tools/cargo/verify.py tools/cargo/target/debug/bsmr-cargo <rust-1.97.1-bin-directory>
python3 tools/cargo/verify.py tools/cargo/target/debug/bsmr-cargo <nightly-2026-04-11-bin-directory>
python3 tools/cargo/verify_sources.py tools/cargo/target/debug/bsmr-cargo <rust-1.97.1-bin-directory>
```

`verify.py` compares 21 configured graphs to the selected Cargo CLI. It covers
build/test/check modes, release profiles, explicit root filters, feature
separation, package environment, declared features, compiler hooks, storage
ownership, and lock preservation. Its Rust sources deliberately do not compile.
Three cases select two packages together and require shared features to match
Cargo in development, test, and release profiles. Empty package lists fail.
Two more cases select equally named binaries from two packages without including
their unrequested roots. Empty target lists and mismatched packages fail.

`python3 tools/cargo/fixture.py tools/cargo/target/debug/bsmr-cargo` regenerates
the native-lowering fixture. Only workspace placement and the diagnostic
compiler banner are normalized.

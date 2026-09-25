---
id: cargo
title: Rust and Cargo
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Documents the native Cargo workspace API and its current hermeticity boundary. -->

# Rust and Cargo

BSMR compiles Cargo packages through native Rust actions. Configured Cargo
planning extends the native frontend released in version 0.0.6.

Build a Cargo package without writing or synchronizing build rules:

```console
bsmr init
bsmr build app --show-output
bsmr test app
```

Run `bsmr build --help` or `bsmr test --help` for command options.

Select Cargo profiles and features through the existing configuration interface:

```console
bsmr build app -c rust.profile=release
bsmr test app -c rust.features=app/test-hooks
bsmr build app -c rust.default_features=false -c rust.all_features=true
```

The `rust` configuration section accepts `profile`, `features`, `default_features`,
and `all_features`. Profiles default to `dev` for builds and `test` for tests.
Features use Cargo's comma- or space-separated syntax. Default features are enabled
and all-features selection is disabled unless requested. Cargo validates names
and resolves profile inheritance and dependency features. Changes invalidate the
selected plan.

Cargo profiles can select `lto = "thin"`, `"fat"`, or `true` for optimization
across crates. Bessemer retains LLVM bitcode in dependency libraries and applies
the selected mode when linking binaries, C libraries, or inline unit tests. Rust library artifacts
also retain object code so they remain usable by consumers that do not use LTO.
`false` permits optimization within one crate. `"off"` disables it entirely.
See [Cargo's LTO settings](https://doc.rust-lang.org/cargo/reference/profiles.html#lto).

Commit `Cargo.toml`, `Cargo.lock`, and an exact `rust-toolchain.toml` at the
project root. The current catalog supports `1.97.1`, `1.98.0`, and `nightly-2026-04-11` on
Linux and macOS, for ARM64 and x86-64. Install the matching Cargo resolver with
`rustup toolchain install <channel> --profile minimal` before building.

```toml title="rust-toolchain.toml"
[toolchain]
channel = "1.97.1"
profile = "minimal"
```

BSMR reads manifests, project Cargo configuration, and target file names into an
isolated snapshot. It discovers public targets without resolving their dependencies,
then asks the bundled Cargo 0.98.0 planner for the selected build or test. The
compiler pin is independent of this resolver version.

Planning may acquire locked dependencies into BSMR's own Cargo home. It never
compiles a build script or loads a procedural macro. BSMR keeps generated build
rules in memory and writes no `BUILD.bsmr` files into the checkout. Resolver input
changes invalidate planning. Ordinary source edits invalidate compiler actions.

```text
Cargo files -> locked resolution -> private targets -> native rustc actions
                                                 -> custom recipe actions
```

Each crate uses the existing native Rust compilation, linking, and test machinery.
A package containing one library or binary can be selected by its directory,
even when its Cargo name differs. Library builds preserve `lib`, `rlib`, `cdylib`,
and `staticlib` declarations. All declared formats materialize even when the
library is only a dependency of the selected executable. `:lib[cdylib]` and
`:lib[staticlib]` select individual C-compatible artifacts. `:lib[check]` requests
metadata only. Libraries expose `:lib`, binaries expose
their Cargo target name, and dependency
renames preserve the name used in source. Package metadata enters the compiler
as literal environment values, so text such as `$(location ...)` cannot become
a build dependency. Inline unit tests and integration tests are associated with
their build targets. Changing an unrelated crate does not recompile the selected
binary. Changing a dependency invalidates its consumers.

## Add another build step

A [custom recipe](../../recipes.md) can consume an inferred Rust executable
without restating its crate dependencies. Keep the recipe in its own package
so the Rust package retains its inferred definition.

## Supported boundary

The frontend supports local path, public registry, and pinned Git libraries,
binaries, inline unit tests, and integration tests. Registry archives are verified against
`Cargo.lock`. Git packages are read from the locked commit without ambient
Git filters or hooks. Compilation reads native source artifacts, not Cargo's
mutable checkout cache. The planner exports pinned Git objects to checksum-verified
archives, so compiler actions need no Git fetch or network access. Git submodules
and authenticated registries remain unsupported.
Excluded path dependencies provide source inputs without becoming public workspace targets.
Cargo resolves requested features, conditional dependencies, dev dependencies, profile
settings, and workspace lints before native compilation. Each build or test has a
separate configured graph, so building a library does not activate its test-only
dependencies.

Build scripts and procedural macros require the
[verified Linux namespace runtime](https://github.com/dedalus-labs/bsmr/blob/main/app/bsmr_execute_impl/src/executors/namespace/README.md).
Unisolated execution rejects package code before compilation. This does not
establish macOS package-code isolation or complete C/C++ toolchain support.

Cargo's `links` metadata flows only to direct dependents' build scripts.
Generated directory paths remain attached to their producing artifacts and are
rebound when cached outputs are restored. Metadata emission order is preserved,
including keys that become identical after Cargo's environment-name conversion.
Declaring `links` does not itself require a C compilation or linker invocation.

Generic `rustc-link-arg` directives reach the package's compiler actions through
the existing argument file. Rustc applies them when linking and ignores them for
`rlib` compilation. Target-specific linker directives remain unsupported.

Cargo's `target.<triple>.linker` and `target.'cfg(...)'.linker` select the driver.
`-C link-arg=...` passes arguments unchanged. For example:

```toml title=".cargo/config.toml"
[target.'cfg(target_os = "linux")']
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=wild"]
```

Configured linking requires the verified namespace runtime. Include the driver,
linker and their libraries in that pinned runtime. Its content digest enters
action identity. BSMR preserves `RUSTC_LINKER` for build scripts and uses the
existing native C++ toolchain for linking. A missing driver or linker fails.
There is no linker-name allowlist or automatic replacement.

Experimental linkers use the same explicit configuration and do not change the
default. The [mold Mach-O port](https://github.com/rui314/mold-macho) accepts
Clang's `--ld-path` selection. Native macOS configured-linker execution remains
unqualified, as does macOS package-code isolation.

Cross compilation remains unsupported. Project compiler flags otherwise support
cfg values, lints, and scalar optimization settings.
Response files, compiler extensions, file-based overrides, and Cargo
CLI option parity are not implemented. Shared files outside a crate require
explicitly declared action inputs.

Repeated builds of the same entrypoint reuse unchanged actions, including across
checkouts. Distinct entrypoints currently own separate configured graphs and may
compile an otherwise identical shared library again.

Stable toolchain pins reject nightly-only language features. Stable builds reuse
compiled libraries for the metadata needed by dependent crates. Nightly builds
retain separate metadata actions so dependent compilation can start sooner.

Native actions validate rustc's reported source and environment reads before
accepting a successful compiler result. Undeclared reported inputs fail instead of
publishing an incomplete cache entry. Declared directories cover their resolved
contents. Symlink targets outside those directories must be declared separately.
This check is not a filesystem sandbox.

Rust compiler, Clippy, and standard-library archives have pinned SHA-256 digests.
Their pinned sizes let the download rule use cached compiler archives without
querying their HTTP headers.
Their content contributes to compilation action identity. Target discovery uses
the selected local rustup installation, while configured planning uses the
bundled resolver. C/C++ linking and Python bootstrap tools
still come from the execution host. This is not a fully hermetic or remotely
qualified toolchain. Native actions honor the existing cache-upload policy.

Ambient Rust flags and user Cargo configuration do not configure native actions.
Project configuration supplies the planner's flags and profiles. Project `[env]`
values are not supported yet.
Unsupported projects can use Cargo directly. BSMR never silently switches to a
whole-workspace Cargo build after inference or compilation fails.

[RFC 0002](https://github.com/dedalus-labs/bsmr/discussions/14) describes the
broader Rust design.

## Verification

`test/native-rust-build.ts` exercises real compiler actions, unit-test success and
failure, and a custom recipe consuming an inferred executable. It checks
manifest invalidation, source invalidation, unrelated edits, cache restoration
in a second checkout, environment isolation, and build-script rejection. It also
checks that inference leaves the checkout free of generated build files.

`python3 test/rust/linker.py /path/to/bsmr /path/to/runtime.json clang -- -fuse-ld=wild`
compares the selected driver with Cargo. It checks build-script environment,
warm reuse, changed linker arguments and missing tools without substitution.

`test/rust/metadata.py` compares metadata visibility and ordering with Cargo. It
checks generated paths, cache restoration across checkouts, and source edits.

`bsmr test app` includes the package's integration tests. Cargo supplies their
dev dependencies and the binaries exposed through `CARGO_BIN_EXE_<name>`.
Tests run from their declared package source directory. Artifact-backed
environment paths remain absolute when a test changes its working directory.
Local packages in the selected dependency graph retain their workspace layout
in a declared test resource. A test can read a sibling package's fixtures through
the same relative path used by Cargo. The primary package retains source changes
produced by its build script. The view remains read-only and tracks fixture edits.
When that directory is a mapped source root, `file!()` paths are relative to it.
Snapshot readers can locate the checked-in files without exposing the generated
target directory or making source inputs writable.
Custom harnesses use their own `main` with Cargo's `cfg(test)` setting.

`python3 test/rust/tests.py /path/to/bsmr` compares these contracts with Cargo.
Supply `/path/to/runtime.json` as a second argument to select the Linux namespace
runtime explicitly. The fixture checks test-only
features, binary execution, fixture reads and failing custom harnesses. It
does not qualify macOS package-code isolation or arbitrary host dependencies.

`test/rust/lto.ts` compares release LTO with Cargo across a three-crate chain.
It checks compiler flags, dependency edits, cached output restoration, and unit tests.

`test/rust/libraries.ts` compares Cargo's multiple-output behavior with native
builds. It runs a Rust consumer and loads the generated C library before and after
a source edit, then checks warm reuse and restoration after cleaning outputs.

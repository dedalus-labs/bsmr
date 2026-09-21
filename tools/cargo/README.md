<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Describes the internal configured-unit planning boundary. -->

# Cargo planning

`bsmr-cargo` reads one JSON request from stdin and writes Cargo's configured
version-1 configured graph to stdout. Cargo resolves features, profiles, host/target units, and
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
the duration of the request. Planning requires an existing lockfile and frozen,
offline sources. `source_policy` must be `offline`.

`package`, `mode`, and `target_filter` select the roots. Modes are `build`,
`test`, and `check`. Filters are `package`, `library`, `binary` with a `name`,
and `integration-test` with a `name`. The request also supplies `features`,
`default_features`, `all_features`, `profile`, and a target triple or null.
Unknown request fields and conflicting compiler overrides fail. Compiler flags
are limited to cfg values, lints, and scalar optimization settings. Response
files, compiler extensions, and file-based overrides fail before compiler probes.
Inactive target tables may carry linker arguments, but selected units using them
require a declared native execution contract.

Each unit includes its source identity, package environment, declared and active
features, compiler and documentation flags, lints, effective linker, and resolved
dependency aliases. The response reports the resolver and actual compiler
versions. External source verification is required before external packages can
enter this graph.

The resolver pins Cargo 0.98.0 and admits Rust 1.97.1 and
nightly-2026-04-11. Its standalone workspace isolates Cargo's native dependencies
from the build engine. See [Cargo's unit graph](https://doc.rust-lang.org/cargo/reference/unstable.html#unit-graph).

```console
cargo +1.98.0 build --manifest-path tools/cargo/Cargo.toml --locked -j 2
python3 tools/cargo/check.py tools/cargo/target/debug/bsmr-cargo
```

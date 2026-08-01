<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# Bessemer

Bessemer (`bsmr`) is a fast, extensible build system derived from
[Buck2](https://github.com/facebook/buck2).

The project is under active development and has not published a stable release.

## Build

The repository pins its Rust toolchain in
[`rust-toolchain.toml`](rust-toolchain.toml).

```sh
cargo build --locked --bin bsmr
target/debug/bsmr --version
```

## Test

```sh
python3 test.py --ci --git --bsmr=target/debug/bsmr
```

The GitHub workflows are generated from typed
[Hollywood](https://github.com/dedalus-labs/hollywood) sources:

```sh
corepack enable
pnpm install --frozen-lockfile --ignore-scripts
pnpm run ci check
```

## Upstream

Bessemer preserves upstream copyright notices and licenses. The `upstream`
Git remote should point to `https://github.com/facebook/buck2.git`; pushes to
that remote should remain disabled.

## License

Except where an inherited notice states otherwise, Bessemer is licensed under
the [Apache License 2.0](LICENSE).

Buck2-derived and third-party files retain their original copyright and
license notices. [LICENSE-MIT](LICENSE-MIT) records Meta's upstream grant; it
does not license Bessemer-authored additions under MIT. See [NOTICE](NOTICE)
for attribution details.

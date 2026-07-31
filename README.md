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

Bessemer's collective work and Dedalus-authored contributions are licensed
under the [Apache License 2.0](LICENSE-APACHE).

Buck2-derived and third-party files retain their original copyright and
license notices. Buck2 files remain available under Meta's MIT OR Apache-2.0
grant, so [LICENSE-MIT](LICENSE-MIT) remains for those upstream files. See
[NOTICE](NOTICE) for attribution details.

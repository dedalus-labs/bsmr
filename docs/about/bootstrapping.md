---
id: bootstrapping
title: Bootstrapping Bessemer
---

# Bootstrapping Bessemer

Bessemer can be built with `cargo` or Bessemer itself. The source repository
includes an upstream [Buck2](https://github.com/facebook/buck2) binary through
[DotSlash](https://dotslash-cli.com), so a clean checkout can build its first
`bsmr` binary.

For dependencies on Rust crates from [crates.io](https://crates.io), we use
[reindeer](https://github.com/facebookincubator/reindeer) to automatically
generate `BUCK` files.

Note that the resulting binary will be compiled without optimisations or
[jemalloc](https://github.com/jemalloc/jemalloc), so we recommend using the
Cargo-produced binary in further development.

First, install `dotslash` with `Cargo`:

```sh
cargo install --locked dotslash
```

Next, use `reindeer` to buckify dependencies:

```sh
cd bsmr/
./tools/bin/reindeer --third-party-dir tools/build/third-party/rust buckify
```

Build the first copy of `bsmr` with the upstream bootstrap binary:

```sh
./tools/bootstrap/upstream-buck2 build //:bsmr
```

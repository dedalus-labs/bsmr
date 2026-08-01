<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

## Build bsmr with Cargo

From bsmr project root, run the following to build bsmr with cargo

```sh
cargo install --path=app/bsmr --root=/tmp
export BSMR="/tmp/bin/bsmr"
```

## Run `bsmr init --git`

Run `bsmr init` to initialize the prelude directory.

Now all targets aside from OCaml related ones are ready to be built.

## Support for building the example OCaml targets

The information in this section is (at this time) Linux and macOS specific.

The commands in `ocaml-setup.sh` assume an activated
[opam](https://opam.ocaml.org/) installation. Their effect is to create a
symlink in the 'third-party/opam' directory. This symlink supports building the
example OCaml targets. If the symlink is found to already exist, it will not be
overwritten.

## Sample commands

**_NOTE:_** These commands are currently only supported on Linux and macOS.

```sh
$BSMR build //ocaml/...
$BSMR run //python/hello_world:main
```

<!-- ===----------------------------------------------------------------------=== -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# bsmr examples

In these folders are some examples on how to get bsmr working with your
favorite languages and tools.

## with_prelude

Examples taking advantage of the prelude to create toolchain-independent build
definitions in cpp and python. Includes as an example a usecase for building and
using c-extension-backed python libraries.

Note: to take advantage of these examples you must symlink the prelude into this
folder.

## no_prelude

Preludeless examples for those wanting to use bsmr with their own rules and
toolchains. In here you can learn about how BUILD files interact with rules, and
how the provider abstraction can be used to encapsulate build logic.

## toolchains

Examples testing the various toolchains included in the prelude.

## bootstrap

A sample project that demonstrates configuration of a bootstrap toolchain.

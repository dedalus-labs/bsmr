<!-- ===----------------------------------------------------------------------=== -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# bsmr e2e tests

This directory contains tests for bsmr. Primary constituents:

 - `core/` - the primary and fully endorsed set of integration tests for bsmr core code. If you're
   working on bsmr itself, this is probably what you want.
 - `e2e_util/` - the test framework for the integration tests.
 - `e2e/` and `meta_only/e2e` - a hodgepodge of tests covering a combination of bsmr itself, the
   prelude, some macros, and various integrations. Avoid if possible. Strongly avoid in favor of
   `core/` if testing bsmr core.
 - `targets/` - target definitions accessed by `e2e` tests.
 - `prelude/` - there is currently no fully endorsed testing strategy for the prelude. This
   directory is an attempt at creating one, however its still immature and there are gaps.
   Trendsetters are welcome to try it.
 - An assortment of other things that mostly shouldn't be here.

`core` and `prelude` tests are visible in open source but not executed there.

Some of these directories have their own `README.md` files.

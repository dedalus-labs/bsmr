# ===----------------------------------------------------------------------===
# Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc
# Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# Stub `testpilot_defs.bzl` for the OSS bsmr build.
#
# `prelude/toolchains/android/test/com/dedalus/bsmr/testrunner/BUILD.bsmr`
# loads this file from `@bsmr_build//rules:testpilot_defs.bzl` to
# get a `tpx_labels` struct (used as `labels = [tpx_labels.long_running]`).
# `fbsource` resolves to the build-support cell, so the load
# resolves here. The fbcode-internal version provides Test Pilot label
# constants; in OSS we don't run via Test Pilot, so just expose the
# string literals the prelude references so that BUILD.bsmr file parses.

tpx_labels = struct(
    long_running = "long_running",
)

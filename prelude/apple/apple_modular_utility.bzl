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

# We use a fixed module cache location. This works around issues with
# multi-user setups with MobileOnDemand and allows us to share the
# module cache with Xcode, LLDB and arc focus.
#
# TODO(T123737676): This needs to be changed to use $TMPDIR in a
# wrapper for modular clang compilation.
MODULE_CACHE_PATH = "/tmp/bsmr-module-cache"

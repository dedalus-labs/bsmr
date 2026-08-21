# ===----------------------------------------------------------------------===
# Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc
# Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

def rust_library_from_crates(name):
    # @lint-ignore BSMRLINT: avoid "Direct usage of native rules is not allowed."
    native.export_file(name = name, src = "BUILD.bsmr", visibility = ["PUBLIC"])

def rust_binary_from_crates(name):
    # @lint-ignore BSMRLINT: avoid "Direct usage of native rules is not allowed."
    native.genrule(name = name, cmd = "exit 1", executable = True, out = "out", visibility = ["PUBLIC"])

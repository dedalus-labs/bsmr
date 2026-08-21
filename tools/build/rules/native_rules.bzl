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

def bsmr_genrule(visibility = ["PUBLIC"], **kwargs):
    # @lint-ignore BSMRLINT: avoid "native is forbidden in fbcode"
    native.genrule(visibility = visibility, **kwargs)

def bsmr_filegroup(visibility = ["PUBLIC"], **kwargs):
    # @lint-ignore BSMRLINT: avoid "native is forbidden in fbcode"
    native.filegroup(visibility = visibility, **kwargs)

def alias(actual, visibility = ["PUBLIC"], **kwargs):
    if actual.startswith("root//"):
        actual = "root//" + actual.removeprefix("root//")
    native.alias(actual = actual, visibility = visibility, **kwargs)

def bsmr_sh_binary(visibility = ["PUBLIC"], **kwargs):
    # @lint-ignore BSMRLINT: avoid "native is forbidden in fbcode"
    native.sh_binary(visibility = visibility, **kwargs)

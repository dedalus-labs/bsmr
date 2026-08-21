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


import os

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test

# TODO(nga): Local and remote execution of `//:dog_and_bone` must produce identical output.
#   This is a known limitation of at least our RE implementation. It reads through symlinks.


@bsmr_test(skip_for_os=["windows"])
async def test_symlink_preserves_empty_directory_local(bsmr: Bsmr) -> None:
    result = await bsmr.build("//:dog_and_bone", "--prefer-local", "--no-remote-cache")
    out = result.get_build_report().output_for_target("//:dog_and_bone")
    assert os.path.islink(out)
    assert os.path.isfile(out)


@bsmr_test(skip_for_os=["windows"])
async def test_symlink_preserves_empty_directory_remote(bsmr: Bsmr) -> None:
    result = await bsmr.build("//:dog_and_bone", "--prefer-remote")
    out = result.get_build_report().output_for_target("//:dog_and_bone")
    # This is incorrect, should be a symlink.
    assert not os.path.islink(out)
    assert os.path.isfile(out)


@bsmr_test()
async def test_noop(bsmr: Bsmr) -> None:
    return

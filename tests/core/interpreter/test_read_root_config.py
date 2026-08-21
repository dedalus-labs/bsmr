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

# pyre-strict


from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_read_root_config(bsmr: Bsmr) -> None:
    output = await bsmr.build("//:")
    assert "<<root=regular>>" in output.stderr
    assert "<<root_ignore_default=regular>>" in output.stderr
    assert "<<root_use_default=predict>>" in output.stderr
    assert "<<local=regular>>" in output.stderr

    output = await bsmr.build("other//:")
    assert "{{root=regular}}" in output.stderr
    assert "{{root_ignore_default=regular}}" in output.stderr
    assert "{{root_use_default=quantity}}" in output.stderr
    assert "{{local=guerrilla}}" in output.stderr

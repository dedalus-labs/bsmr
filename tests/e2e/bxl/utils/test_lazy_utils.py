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


from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test(inplace=False, data_dir="bxl/simple", skip_for_os=["windows"])
async def test_catch_resolve_lazy_dict(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl/utils:test_lazy_utils.bxl:test_catch_resolve_lazy_dict",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple", skip_for_os=["windows"])
async def test_batch_apply_lazy(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl/utils:test_lazy_utils.bxl:test_batch_apply_lazy",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple", skip_for_os=["windows"])
async def test_batch_apply_lazy_catch_each(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl/utils:test_lazy_utils.bxl:test_batch_apply_lazy_catch_each",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple", skip_for_os=["windows"])
async def test_batch_apply_lazy_catch_all(bsmr: Bsmr) -> None:
    await bsmr.bxl("//bxl/utils:test_lazy_utils.bxl:test_batch_apply_lazy_catch_all")


@bsmr_test(inplace=False, data_dir="bxl/simple", skip_for_os=["windows"])
async def test_partition_results(bsmr: Bsmr) -> None:
    await bsmr.bxl("//bxl/utils:test_lazy_utils.bxl:test_partition_results")


@bsmr_test(inplace=False, data_dir="bxl/simple", skip_for_os=["windows"])
async def test_partition_results_dict(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl/utils:test_lazy_utils.bxl:test_partition_results_dict",
    )


# dummy test to avoid test listing failure on windows
@bsmr_test(inplace=True)
async def test_dummy(bsmr: Bsmr) -> None:
    pass

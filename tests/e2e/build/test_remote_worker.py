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
from bsmr.tests.e2e_util.helper.utils import (
    expect_exec_count,
    random_string,
    read_what_ran,
)


@bsmr_test(inplace=True)
async def test_remote_worker(bsmr: Bsmr) -> None:
    target = "root//tests/targets/rules/remote_worker:run_two_worker_rules"
    result = await bsmr.build(
        target,
        "-c",
        f"test.cache_buster={random_string()}",
    )

    output = result.get_build_report().output_for_target(target)

    output_path = bsmr.cwd / output
    with open(output_path, "r") as f:
        output_lines = f.readlines()
        assert len(output_lines) == 2
        # We would like to check that both lines are the same, as that means that
        # both actions used the same persistent worker.
        # However, RE just does a best effort to use the same persistent worker,
        # so we can't guarantee this is the case.
        # assert output_lines[0].strip() == output_lines[1].strip()

    whatran_json = await read_what_ran(bsmr)
    worker_entries = [x for x in whatran_json if "run_remote_worker" in x["identity"]]
    assert len(worker_entries) == 2, whatran_json
    assert worker_entries[0]["reproducer"]["executor"] == "ReWorker"

    whatran = (await bsmr.log("what-ran", "--skip-cache-hits")).stdout.split("\n")
    worker_lines = [x for x in whatran if "re_worker(" in x]
    assert len(worker_lines) == 2, whatran


@bsmr_test(inplace=True)
async def test_remote_worker_caches(bsmr: Bsmr) -> None:
    target = "root//tests/targets/rules/remote_worker:run_remote_worker_1"
    args = [
        target,
    ]
    await bsmr.build(*args)

    bsmr.kill()

    await bsmr.build(*args)
    await expect_exec_count(bsmr, 0)

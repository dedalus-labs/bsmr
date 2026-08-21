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


import json
import re
import typing

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


# Obtain hashes of `<astrologer>` and `<vagabond>` configurations.
async def _obtain_cfg_hashes(bsmr: Bsmr) -> typing.Tuple[str, str]:
    result = await bsmr.cquery(
        "root//:simple",
        "--target-universe",
        "root//:universe",
    )
    [astrologer, vagabond] = result.stdout.splitlines()
    assert astrologer.startswith("root//:simple (<astrologer>#")
    assert vagabond.startswith("root//:simple (<vagabond>#")
    astrologer_hash = re.sub(r".*#(.*)\)", r"\1", astrologer)
    vagabond_hash = re.sub(r".*#(.*)\)", r"\1", vagabond)
    assert re.fullmatch("[0-9a-f]{16}", astrologer_hash), astrologer
    assert re.fullmatch("[0-9a-f]{16}", vagabond_hash), vagabond
    return (astrologer_hash, vagabond_hash)


@bsmr_test()
async def test_build_configured_full_configuration(bsmr: Bsmr) -> None:
    (astrologer_hash, _) = await _obtain_cfg_hashes(bsmr)

    result = await bsmr.build(
        f"root//:simple (<astrologer>#{astrologer_hash})",
        "--target-universe",
        "root//:universe",
    )
    out = result.get_build_report().output_for_target("root//:simple").read_text()
    assert f"$$$root//:simple (<astrologer>#{astrologer_hash})$$$" == out


@bsmr_test()
async def test_build_configured_no_hash(bsmr: Bsmr) -> None:
    (_, vagabond_hash) = await _obtain_cfg_hashes(bsmr)
    result = await bsmr.build(
        "root//:simple (<vagabond>)",
        "--target-universe",
        "root//:universe",
    )
    out = result.get_build_report().output_for_target("root//:simple").read_text()
    assert f"$$$root//:simple (<vagabond>#{vagabond_hash})$$$" == out


@bsmr_test()
async def test_build_configured_wrong_hash(bsmr: Bsmr) -> None:
    result = await bsmr.build(
        "root//:simple (<vagabond>#0123456789abcdef)",
        "--target-universe",
        "root//:universe",
    )
    # TODO(nga): this should either fail or emit a warning.
    assert "root//:simple" not in json.loads(result.stdout)["results"]


@bsmr_test()
async def test_build_configured_no_universe(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.build(
            "root//:simple (<vagabond>)",
        ),
        stderr_regex="Targets with explicit configuration can only be built when the",
    )

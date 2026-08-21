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


import sys

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.utils import json_get, random_string


async def _assert_locally_executed_upload_attempted(bsmr: Bsmr, count: int = 1) -> None:
    await _assert_upload_attempted(bsmr, count)


async def _assert_upload_attempted(bsmr: Bsmr, count: int) -> None:
    log = (await bsmr.log("show")).stdout.strip().splitlines()
    uploads = []
    excluded_uploads = []

    # CI lacks reliable write access to CAS, so count any upload that was
    # *attempted* — both genuine successes and infra-level rejections that
    # prove the action reached the cache-upload stage. PERMISSION_DENIED is
    # the missing-write-ACL case. INVALID_ARGUMENT is only tolerated when the
    # message identifies the specific "Outputs TTL -1 is too low" rejection
    # (raised when supporting CAS objects haven't been uploaded).
    for line in log:
        e = json_get(
            line,
            "Event",
            "data",
            "SpanEnd",
            "data",
            "CacheUpload",
        )
        if e is None:
            continue
        tolerated = (
            e["success"]
            or e["re_error_code"] == "PERMISSION_DENIED"
            or (
                e["re_error_code"] == "INVALID_ARGUMENT"
                and "Outputs TTL -1 is too low" in e.get("error", "")
            )
        )
        if tolerated:
            uploads.append(e)
        else:
            excluded_uploads.append(e)

    if len(uploads) == count:
        return
    else:
        print(f"Expected {count} uploads", file=sys.stderr)
        print(f"Actual uploads: {uploads}", file=sys.stderr)
        print(f"Excluded uploads: {excluded_uploads}", file=sys.stderr)
        raise AssertionError("Wrong number of uploads, see above")


@bsmr_test()
async def test_re_uploads(bsmr: Bsmr) -> None:
    args = ["-c", f"write.text={random_string()}"]
    await bsmr.build("root//:write", *args)
    await _assert_locally_executed_upload_attempted(bsmr, 1)


@bsmr_test()
async def test_re_uploads_dir(bsmr: Bsmr) -> None:
    args = ["-c", f"write.text={random_string()}"]
    await bsmr.build("root//:write_in_dir", *args)
    await _assert_locally_executed_upload_attempted(bsmr, 1)


@bsmr_test()
async def test_re_uploads_limit(bsmr: Bsmr) -> None:
    args = ["-c", f"write.text={random_string()}"]
    await bsmr.build("root//:write_xxl", *args)
    await _assert_locally_executed_upload_attempted(bsmr, 0)


@bsmr_test()
async def test_re_uploads_default(bsmr: Bsmr) -> None:
    args = ["-c", f"write.text={random_string()}"]
    await bsmr.build("root//:write_default", *args)
    await _assert_locally_executed_upload_attempted(bsmr, 0)

    args = [
        "-c",
        f"write.text={random_string()}",
        "-c",
        "bsmr.default_allow_cache_upload=true",
    ]
    await bsmr.build("root//:write_default", *args)
    await _assert_locally_executed_upload_attempted(bsmr, 1)

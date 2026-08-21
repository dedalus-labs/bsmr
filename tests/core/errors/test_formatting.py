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


import re

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.golden import golden, strip_glog_lines


def _sanitize(s: str) -> str:
    s = strip_glog_lines(s)
    # Remove configuration hashes
    s = re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)
    # And action digests
    s = re.sub(r"\b[0-9a-f]{40}:[0-9]{1,3}\b", "<DIGEST>", s)
    return s


def error_formatting_test(
    name: str, command: list[str], command_name: str = "build"
) -> None:
    async def impl(bsmr: Bsmr) -> None:
        func = getattr(bsmr, command_name)
        res = await expect_failure(func("--console=none", *command))
        golden(
            output=_sanitize(res.stderr),
            rel_path="fixtures/" + name + ".golden.stderr",
        )

    globals()[name] = impl

    bsmr_test()(impl)


error_formatting_test(name="test_action_fail", command=["//:action_fail"])

error_formatting_test(
    name="test_missing_dep",
    command=["//:missing_dep"],
)

error_formatting_test(
    name="test_missing_dep_cquery",
    command=["//:missing_dep"],
    command_name="cquery",
)

error_formatting_test(
    name="test_attr_coercion",
    command=["//attr_coercion:int_rule"],
)

error_formatting_test(
    name="test_during_load",
    command=["//during_load:whatever"],
)

error_formatting_test(
    name="test_during_load_via_dep",
    command=["//during_load/via_dep:via_dep"],
)

error_formatting_test(
    name="test_during_parse",
    command=["//during_parse:whatever"],
)

error_formatting_test(
    name="test_during_select_map",
    command=["//during_select:map"],
)

error_formatting_test(
    name="test_bxl_no_stacktrace",
    command=["//fail_no_stacktrace.bxl:fail_no_stacktrace_test"],
    command_name="bxl",
)

error_formatting_test(
    name="test_bxl_no_stacktrace_verbose",
    command=["//fail_no_stacktrace.bxl:fail_no_stacktrace_test", "-v5"],
    command_name="bxl",
)

error_formatting_test(
    name="test_bxl_with_stacktrace",
    command=["//fail_no_stacktrace.bxl:fail_with_stacktrace_test"],
    command_name="bxl",
)

error_formatting_test(
    name="test_bxl_attr_coercion",
    command=["//fail_attr_coercion.bxl:int_rule"],
    command_name="bxl",
)

error_formatting_test(
    name="test_duplicate_target",
    command=["//duplicate_target:foo"],
)

error_formatting_test(
    name="test_duplicate_target_with_stacktrace",
    command=["//duplicate_target:foo", "--stack"],
)

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
from bsmr.tests.e2e_util.helper.utils import filter_events


@bsmr_test()
async def test_peak_allocated_bytes(bsmr: Bsmr) -> None:
    await bsmr.uquery("//:EEE")
    span_end_load_event = await filter_events(
        bsmr,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "Load",
    )
    assert len(span_end_load_event) == 1
    starlark_peak_allocated_bytes = span_end_load_event[0][
        "starlark_peak_allocated_bytes"
    ]
    # list occupies pointer size (8) * number of elements (~10MB) + some extra overhead for bookkeeping
    assert starlark_peak_allocated_bytes >= (8 * 10 * 1 << 20)
    # check that it is no more than +10%
    assert starlark_peak_allocated_bytes < (8 * 11 * 1 << 20)

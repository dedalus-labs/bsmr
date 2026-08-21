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

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.utils import filter_events


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


@bsmr_test()
async def test_configuration_transition_rule_cquery(bsmr: Bsmr) -> None:
    # For the reference, cquery output is: P467297091. Note the "forward" node.
    result = await bsmr.cquery("deps(root//:the-test)")
    result.check_returncode()
    # Watchos resource should be present twice: as forward and as transitioned.
    assert result.stdout.count(":watchos-resource") == 2
    # No transition for default resource, so it appears once in cquery output.
    assert result.stdout.count(":default-resource") == 1


@bsmr_test()
async def test_configuration_transition_rule_cquery_actual_attr(bsmr: Bsmr) -> None:
    result = await bsmr.cquery(
        "--target-platforms=root//:iphoneos-p",
        "root//:watchos-resource",
        "--output-attribute=actual",
    )
    result.check_returncode()
    q = json.loads(result.stdout)

    # Each key in the JSON output is a different configuration of the same rule `watchos-resource`
    configuration_default = "root//:watchos-resource (<transitioned-to-watch>#<HASH>)"
    configuration_transition = "root//:watchos-resource (root//:iphoneos-p#<HASH>)"
    configurations = [_replace_hash(c) for c in q.keys()]
    assert configuration_default in configurations
    assert configuration_transition in configurations

    config_default_has_attribute_actual = False
    config_transition_has_no_attributes = False
    for config in q.keys():
        if q[config].get("actual"):
            config_default_has_attribute_actual = True
        if not q[config].values():
            config_transition_has_no_attributes = True

    assert config_default_has_attribute_actual
    assert config_transition_has_no_attributes


@bsmr_test()
async def test_configuration_transition_rule_build(bsmr: Bsmr) -> None:
    # Rule implementations do the assertions.
    result = await bsmr.build("root//:the-test")
    result.check_returncode()


@bsmr_test()
async def test_configuration_transition_yields_multiple_configurations_created_events(
    bsmr: Bsmr,
) -> None:
    await bsmr.build("root//:the-test")
    configuration_created_events = await filter_events(
        bsmr, "Event", "data", "Instant", "data", "ConfigurationCreated", "cfg"
    )

    assert len(configuration_created_events) == 2
    configuration_names = [cfg["full_name"] for cfg in configuration_created_events]
    assert configuration_names[0].startswith("root//:iphoneos-p")
    assert configuration_names[1].startswith("<transitioned-to-watch>")

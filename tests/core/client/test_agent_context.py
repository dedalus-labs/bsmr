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
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_agent_context_valid_non_enforced(bsmr: Bsmr) -> None:
    """Non-enforced client with valid agent context should succeed."""
    await bsmr.build(
        "//:pass",
        "--agent-context",
        "intent=build,attempt=1",
    )


@bsmr_test()
async def test_agent_context_non_enforced_no_validation(bsmr: Bsmr) -> None:
    """Non-enforced client with invalid values should still succeed (stored as-is)."""
    await bsmr.build(
        "//:pass",
        "--agent-context",
        "intent=garbage_value,attempt=1",
    )


@bsmr_test()
async def test_agent_context_enforced_valid(bsmr: Bsmr) -> None:
    """Enforced client with all required fields and valid values should succeed."""
    await bsmr.build(
        "//:pass",
        "--client-metadata",
        "id=test_enforced_client",
        "--agent-context",
        "intent=build,attempt=1",
    )


@bsmr_test()
async def test_agent_context_enforced_with_optional_field(bsmr: Bsmr) -> None:
    """Enforced client with required + optional fields should succeed."""
    await bsmr.build(
        "//:pass",
        "--client-metadata",
        "id=test_enforced_client",
        "--agent-context",
        "intent=fix,attempt=2,prior_error=missing_target",
    )


@bsmr_test()
async def test_agent_context_enforced_invalid_value(bsmr: Bsmr) -> None:
    """Enforced client with invalid value for constrained field should fail
    with the exact error message including valid values and description."""
    await expect_failure(
        bsmr.build(
            "//:pass",
            "--client-metadata",
            "id=test_enforced_client",
            "--agent-context",
            "intent=invalid_value,attempt=1",
        ),
        stderr_regex=(
            r"Invalid agent-context value `invalid_value` for key `intent`\."
            r"\s+intent: The purpose of this bsmr invocation"
            r"\s+Valid values: build, test, query, fix, investigate"
        ),
    )


@bsmr_test()
async def test_agent_context_enforced_unknown_key(bsmr: Bsmr) -> None:
    """Enforced client with unknown key should fail with sorted list of valid keys."""
    await expect_failure(
        bsmr.build(
            "//:pass",
            "--client-metadata",
            "id=test_enforced_client",
            "--agent-context",
            "intent=build,attempt=1,unknown_key=foo",
        ),
        stderr_regex=(
            r"Unknown agent-context key `unknown_key`\."
            r"\s+Valid keys: attempt, intent, prior_error"
        ),
    )


@bsmr_test()
async def test_agent_context_enforced_missing_required(bsmr: Bsmr) -> None:
    """Enforced client missing required fields should list all missing fields sorted."""
    await expect_failure(
        bsmr.build(
            "//:pass",
            "--client-metadata",
            "id=test_enforced_client",
            "--agent-context",
            "prior_error=missing_target",
        ),
        stderr_regex=(
            r"Missing required agent-context field\(s\):"
            r"\s+- attempt: Which attempt number this is for the same logical task"
            r"\s+- intent: The purpose of this bsmr invocation"
        ),
    )


@bsmr_test()
async def test_agent_context_enforced_empty_value_counts_as_missing(
    bsmr: Bsmr,
) -> None:
    """Enforced client with empty value for required field should report it as missing."""
    await expect_failure(
        bsmr.build(
            "//:pass",
            "--client-metadata",
            "id=test_enforced_client",
            "--agent-context",
            "intent=,attempt=1",
        ),
        stderr_regex=(
            r"Missing required agent-context field\(s\):"
            r"\s+- intent: The purpose of this bsmr invocation"
        ),
    )


@bsmr_test()
async def test_agent_context_no_context_passes(bsmr: Bsmr) -> None:
    """Build without --agent-context should always succeed."""
    await bsmr.build("//:pass")


@bsmr_test()
async def test_agent_context_enforced_no_context_passes(bsmr: Bsmr) -> None:
    """Enforced client without --agent-context should succeed (not required in v1)."""
    await bsmr.build(
        "//:pass",
        "--client-metadata",
        "id=test_enforced_client",
    )


@bsmr_test()
async def test_agent_context_invalid_format(bsmr: Bsmr) -> None:
    """Malformed --agent-context should fail with the exact format error."""
    await expect_failure(
        bsmr.build(
            "//:pass",
            "--agent-context",
            "not_a_valid_format",
        ),
        stderr_regex=(
            r"Invalid agent-context format: `not_a_valid_format`\."
            r" Each entry must be a `key=value` pair\."
        ),
    )


@bsmr_test()
async def test_agent_context_invalid_key_format(bsmr: Bsmr) -> None:
    """Non-snake_case key should fail with the exact key format error."""
    await expect_failure(
        bsmr.build(
            "//:pass",
            "--agent-context",
            "InvalidKey=value",
        ),
        stderr_regex=(
            r"Invalid agent-context key: `InvalidKey`\."
            r" Keys must be snake_case identifiers\."
        ),
    )


@bsmr_test(write_invocation_record=True)
async def test_agent_context_logged_to_invocation_record(bsmr: Bsmr) -> None:
    """Agent context should appear in the invocation record."""
    res = await bsmr.build(
        "//:pass",
        "--agent-context",
        "intent=build,attempt=1",
    )

    record = res.invocation_record()
    agent_context = record.get("agent_context")
    assert agent_context is not None, (
        "agent_context should be present in invocation record"
    )

    entry_map = {e["key"]: e["value"] for e in agent_context}
    assert entry_map["intent"] == "build"
    assert entry_map["attempt"] == "1"

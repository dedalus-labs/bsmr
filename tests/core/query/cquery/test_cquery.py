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
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.golden import golden_replace_cfg_hash

"""
Generally we test for basic functionality of things working here and do
more extensive testing in the uquery tests.
"""


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


@bsmr_test(data_dir="unsorted")
async def test_query_inputs(bsmr: Bsmr) -> None:
    result = await bsmr.cquery("""inputs(set(root//bin:the_binary //lib:file1))""")
    assert result.stdout == "bin/TARGETS.fixture\n"


@bsmr_test(data_dir="unsorted")
async def test_query_cell(bsmr: Bsmr) -> None:
    result = await bsmr.cquery("""//stuff:magic""", rel_cwd=Path("special"))
    assert (
        _replace_hash(result.stdout)
        == "special//stuff:magic (root//platforms:platform1#<HASH>)\n"
    )


@bsmr_test(data_dir="unsorted")
async def test_query_relative(bsmr: Bsmr) -> None:
    result = await bsmr.cquery("""...""", rel_cwd=Path("special"))
    assert (
        _replace_hash(result.stdout)
        == "special//stuff:magic (root//platforms:platform1#<HASH>)\n"
    )


@bsmr_test(data_dir="unsorted")
async def test_query_provider_names(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.cquery("'root//bin:the_binary[provider_name]'"),
        stderr_regex="Expected a target pattern without providers",
    )

    await expect_failure(
        bsmr.cquery("'root//bin:the_binary#some_flavor'"),
        stderr_regex="Expected a target pattern without providers",
    )


@bsmr_test(data_dir="unsorted")
async def test_query_print_provider_text(bsmr: Bsmr) -> None:
    out = await bsmr.cquery("%s", "root//bin:the_binary", "--show-providers")
    golden_replace_cfg_hash(
        output=_replace_hash(out.stdout),
        rel_path="unsorted/query_print_provider_text.golden.txt",
    )


@bsmr_test(data_dir="unsorted")
async def test_query_print_provider_json(bsmr: Bsmr) -> None:
    out = await bsmr.cquery("%s", "root//bin:the_binary", "--show-providers", "--json")
    golden_replace_cfg_hash(
        output=_replace_hash(out.stdout),
        rel_path="unsorted/query_print_provider_json.golden.json",
    )


@bsmr_test(data_dir="unsorted")
async def test_query_chunked_stream(bsmr: Bsmr) -> None:
    q = "deps(root//bin:the_binary)"
    result1 = await bsmr.cquery(q)
    await bsmr.kill()
    result2 = await bsmr.cquery(q, env={"BSMR_DEBUG_RAWOUTPUT_CHUNK_SIZE": "5"})
    assert result1.stdout == result2.stdout


@bsmr_test(data_dir="unsorted")
async def test_attributes(bsmr: Bsmr) -> None:
    attrs_out = await bsmr.cquery(
        "--output-attribute",
        "bsmr\\..*",
        "--output-attribute",
        "srcs",
        "set(root//bin:the_binary //lib:file1)",
    )
    attrs_json_out = await bsmr.cquery(
        "--output-attribute",
        "bsmr\\..*",
        "--output-attribute",
        "srcs",
        "--json",
        "set(root//bin:the_binary //lib:file1)",
    )
    # specifying any attrs enables json output
    assert attrs_json_out.stdout == attrs_out.stdout
    attrs_json_out = json.loads(_replace_hash(attrs_json_out.stdout))
    assert {
        "root//bin:the_binary (root//platforms:platform1#<HASH>)": {
            "bsmr.deps": [
                "root//:data (root//platforms:platform1#<HASH>)",
                "root//lib:lib1 (root//platforms:platform1#<HASH>)",
                "root//lib:lib2 (root//platforms:platform1#<HASH>)",
                "root//lib:lib3 (root//platforms:platform1#<HASH>)",
                "root//:foo_toolchain (root//platforms:platform1#<HASH>)",
                "root//:bin (root//platforms:platform1#<HASH>)",
            ],
            "bsmr.execution_platform": "<legacy_global_exec_platform>",
            "bsmr.package": "root//bin:TARGETS.fixture",
            "bsmr.plugins": {},
            "bsmr.target_configuration": "root//platforms:platform1#<HASH>",
            "bsmr.type": "_foo_binary",
            "bsmr.oncall": None,
            "srcs": ["root//bin/TARGETS.fixture"],
        },
        "root//lib:file1 (root//platforms:platform1#<HASH>)": {
            "bsmr.deps": [],
            "bsmr.execution_platform": "<legacy_global_exec_platform>",
            "bsmr.package": "root//lib:TARGETS.fixture",
            "bsmr.plugins": {},
            "bsmr.target_configuration": "root//platforms:platform1#<HASH>",
            "bsmr.type": "_foo_genrule",
            "bsmr.oncall": None,
        },
    } == attrs_json_out


# Tests for "%Ss" uses
@bsmr_test(data_dir="unsorted")
async def test_args_as_set(bsmr: Bsmr) -> None:
    out = await bsmr.cquery("%Ss", "root//bin:the_binary", "//lib:file1")
    assert (
        _replace_hash(out.stdout)
        == "root//bin:the_binary (root//platforms:platform1#<HASH>)\nroot//lib:file1 (root//platforms:platform1#<HASH>)\n"
    )


@bsmr_test(data_dir="unsorted")
async def test_multi_query(bsmr: Bsmr) -> None:
    out = await bsmr.cquery("%s", "root//bin:the_binary", "//lib:file1")
    assert (
        _replace_hash(out.stdout)
        == "root//bin:the_binary (root//platforms:platform1#<HASH>)\nroot//lib:file1 (root//platforms:platform1#<HASH>)\n"
    )


@bsmr_test(data_dir="unsorted")
async def test_query_attrfilter(bsmr: Bsmr) -> None:
    out = await bsmr.uquery(
        "attrfilter(bsmr.package, 'root//bin:TARGETS.fixture',root//bin:the_binary)"
    )
    assert out.stdout.strip() == "root//bin:the_binary"


@bsmr_test(data_dir="multi_query_universe")
async def test_multi_query_universe(bsmr: Bsmr) -> None:
    out = await bsmr.cquery(
        "deps(%s)", "root//:macos-bin", "//:common-dep", "--output-format=json"
    )
    # `common-dep` is configured for linux, so it must not include `only-on-macos` target.
    #   Which would be the case if we constructed universe from all the queries together
    #   instead of separate universes for each query.
    golden_replace_cfg_hash(
        output=_replace_hash(out.stdout),
        rel_path="multi_query_universe/multi_query_universe.golden.json",
    )


@bsmr_test(data_dir="unsorted")
async def test_multi_query_print_provider_text(bsmr: Bsmr) -> None:
    out = await bsmr.cquery(
        "%s", "root//bin:the_binary", "//lib:lib1", "--show-providers"
    )
    golden_replace_cfg_hash(
        output=_replace_hash(out.stdout),
        rel_path="unsorted/multi_query_print_provider_text.golden.txt",
    )


@bsmr_test(data_dir="unsorted")
async def test_multi_query_print_provider_json(bsmr: Bsmr) -> None:
    out = await bsmr.cquery(
        "%s", "root//bin:the_binary", "//lib:lib1", "--show-providers", "--json"
    )

    golden_replace_cfg_hash(
        output=_replace_hash(out.stdout),
        rel_path="unsorted/multi_query_print_provider_json.golden.json",
    )


@bsmr_test(data_dir="visibility")
async def test_visibility(bsmr: Bsmr) -> None:
    for good in [
        "self//:pass1",
        "self//:pass2",
        "self//:pass3",
        "self//:pass4",
    ]:
        out = await bsmr.cquery(good)
        assert good in out.stdout

    for bad in [
        "self//:fail1",
        "self//:fail2",
        "self//:fail3",
        "self//:fail4",
    ]:
        print(bad)
        failure = await expect_failure(bsmr.cquery(bad))
        assert "not visible to `%s`" % bad in failure.stderr


@bsmr_test(data_dir="testsof")
async def test_testsof(bsmr: Bsmr) -> None:
    out = await bsmr.cquery(
        "testsof(//:foo_lib)",
        "--target-platforms",
        "//:platform_default_tests",
    )

    assert "root//:foo_test" in out.stdout
    assert "root//:foo_extra_test" not in out.stdout
    assert "root//:foo_lib" not in out.stdout

    out = await bsmr.cquery(
        "testsof(//:foo_lib)",
        "--target-platforms",
        "//:platform_more_tests",
    )

    assert "root//:foo_test" in out.stdout
    assert "root//:foo_extra_test" in out.stdout
    assert "root//:foo_lib" not in out.stdout


# DICE currently may re-evaluate dead nodes ignoring errors, but it cannot ignore panics.
# The disabling of execution platforms through a bsmrconfig ended up causing a panic
# that was the root cause of non-deterministic bsmr failures on 10% of fbcode TD in S303188.
#
# TODO(scottcao): Disabling execution platforms is a hack that we need to get rid of
# because it's not how bsmr should be used. Get rid of this test case once fbcode TD
# stops disabling execution platforms
@bsmr_test(data_dir="toolchain_deps")
async def test_disabling_of_execution_platforms(bsmr: Bsmr) -> None:
    # Run these commands 10x such that a stress run of 10 on continuous CI would run these commands 100x.
    # If there is a regression then the stress run would for sure detect it.
    for _ in range(10):
        query = "deps(set(tests/...))"
        await bsmr.cquery(query)
        await bsmr.cquery(query, "-c", "build.execution_platforms=")


@bsmr_test(data_dir="deps_query")
async def test_declared_deps_query(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.cquery(
            "root//:declared_deps",
        ),
        stderr_regex="Error parsing target pattern `\\$declared_deps`",
    )


# Tests for intersect and except operators on FileSet, TargetSet, and String types
# These tests verify the fix for https://github.com/facebook/buck2/issues/1109
@bsmr_test(data_dir="set_operators")
async def test_cquery_fileset_intersect(bsmr: Bsmr) -> None:
    """Test FileSet intersect FileSet using inputs()."""
    result = await bsmr.cquery(
        """inputs(root//:lib_a) intersect inputs(root//:lib_b)"""
    )
    assert result.stdout == "common.txt\n"


@bsmr_test(data_dir="set_operators")
async def test_cquery_targetset_except(bsmr: Bsmr) -> None:
    """Test TargetSet except TargetSet using set()."""
    result = await bsmr.cquery(
        """set(root//:lib_a root//:app) except set(root//:app)"""
    )
    assert "root//:lib_a" in result.stdout
    assert "root//:app" not in result.stdout

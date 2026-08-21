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


import random
import re
import string

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_cquery_owner(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/cquery.bxl:owner_test",
    )
    assert (
        _replace_hash(result.stdout)
        == "[root//bin:the_binary (root//platforms:platform1#<HASH>)]\n"
    )

    result = await bsmr.bxl(
        "//bxl/cquery.bxl:owner_with_cell_path_test",
    )
    assert (
        _replace_hash(result.stdout)
        == "[root//bin:the_binary (root//platforms:platform1#<HASH>)]\n"
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_cquery_owner_list(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/cquery.bxl:owner_list_test",
    )
    assert (
        _replace_hash(result.stdout)
        == "[root//bin:the_binary (root//platforms:platform1#<HASH>), root//bin:the_binary_with_dir_srcs (root//platforms:platform1#<HASH>)]\n"
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_cquery_kind(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:cquery.bxl:kind_test",
    )

    assert "foo" in result.stdout
    assert "bar" not in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_cquery_inputs(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:cquery.bxl:inputs_test",
    )

    assert "TARGETS.fixture" in result.stdout
    assert "bxl.FileSet" in result.stdout
    assert "1" in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_cquery_filter(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:cquery.bxl:filter_test",
    )

    assert "root//bin:the_binary" in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_cquery_attrregex_filter(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/cquery.bxl:attrregexfilter_test",
    )

    assert "foo" in result.stdout
    assert "bzzt" in result.stdout
    assert "bar" not in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_cquery_attrfilter(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/cquery.bxl:attrfilter_test",
    )

    assert "foo" in result.stdout
    assert "bzzt" not in result.stdout
    assert "bar" not in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_cquery_nattrfilter(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/cquery.bxl:nattrfilter_test",
    )

    assert "foo" not in result.stdout
    assert "bzzt" in result.stdout
    assert "bar" in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_cquery_rdeps(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/cquery.bxl:rdeps_test",
    )
    assert (
        _replace_hash(result.stdout)
        == "[root//bin:the_binary (root//platforms:platform1#<HASH>), root//lib:lib1 (root//platforms:platform1#<HASH>), root//lib:file1 (root//platforms:platform1#<HASH>)]\n"
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_cquery_deps(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/cquery.bxl:deps_test",
    )
    assert (
        _replace_hash(result.stdout)
        == "[root//bin:the_binary (root//platforms:platform1#<HASH>), root//:data (root//platforms:platform1#<HASH>), root//lib:lib1 (root//platforms:platform1#<HASH>), root//lib:lib2 (root//platforms:platform1#<HASH>), root//lib:lib3 (root//platforms:platform1#<HASH>), root//:foo_toolchain (root//platforms:platform1#<HASH>), root//:bin (root//platforms:platform1#<HASH>)]\n"
    )


@bsmr_test(inplace=False, data_dir="bxl/simple", allow_soft_errors=True)
async def test_cquery_buildfile(bsmr: Bsmr) -> None:
    await bsmr.bxl("//bxl/cquery.bxl:buildfile_test")


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_cquery_incompatible_configured_targets(bsmr: Bsmr) -> None:
    # incompatible target should be skipped and the cquery should return compatible targets
    result = await bsmr.bxl("//bxl/cquery.bxl:incompatible_configured_targets_test")
    assert "Skipped 1 incompatible targets" in result.stderr
    assert "root//incompatible_targets:incompatible" in result.stderr
    assert "root//incompatible_targets:foo" in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_cquery_incompatible_configured_targets_single_label(bsmr: Bsmr) -> None:
    # incompatible target should be skipped and the cquery should not fail
    result = await bsmr.bxl(
        "//bxl/cquery.bxl:incompatible_configured_targets_single_label_test"
    )
    assert "Skipped 1 incompatible targets" in result.stderr
    assert "root//incompatible_targets:incompatible" in result.stderr


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_cquery_incompatible_targets(bsmr: Bsmr) -> None:
    # incompatible target should be skipped and the cquery should not fail
    result = await bsmr.bxl("//bxl/cquery.bxl:incompatible_targets_test")
    assert "Skipped 1 incompatible targets" in result.stderr
    assert "root//incompatible_targets:incompatible" in result.stderr
    assert "root//incompatible_targets:foo" not in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_cquery_incompatible_targets_recursive(bsmr: Bsmr) -> None:
    # incompatible target should be skipped and the cquery should return compatible targets
    result = await bsmr.bxl("//bxl/cquery.bxl:incompatible_targets_test_recursive")
    assert "Skipped 2 incompatible targets" in result.stderr
    assert "root//incompatible_targets:incompatible" in result.stderr
    assert "root//incompatible_targets/inner_folder:incompatible_inner" in result.stderr
    assert "root//incompatible_targets/inner_folder:foo_inner" in result.stdout
    assert "root//incompatible_targets:foo" in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_cquery_configured_label(bsmr: Bsmr) -> None:
    await bsmr.bxl("//bxl/cquery.bxl:cquery_configured_label")


@bsmr_test(inplace=False, data_dir="testsof")
async def test_cquery_testsof(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//cquery.bxl:testsof_test",
    )
    assert "root//:foo_test (root//:platform_default_tests" in result.stdout

    result = await bsmr.bxl(
        "//cquery.bxl:testsof_with_default_target_platform_test",
    )
    assert (
        "root//:foo_test_with_default_platform (root//:foo_test_default_platform"
        in result.stdout
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_allpaths(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:uquery.bxl:allpaths_test",
    )

    assert (
        "[root//graph:one, root//graph:ten, root//graph:eleven, root//graph:two, root//graph:three]\n"
        == result.stdout
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_allpaths_filtered(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:uquery.bxl:allpaths_filtered_test",
    )

    assert "[root//graph:one, root//graph:two, root//graph:three]\n" == result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_lazy_allpaths(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:uquery.bxl:lazy_allpaths_test",
    )

    assert (
        "[root//graph:one, root//graph:ten, root//graph:eleven, root//graph:two, root//graph:three]\n"
        == result.stdout
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_lazy_allpaths_filtered(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:uquery.bxl:lazy_allpaths_filtered_test",
    )

    assert "[root//graph:one, root//graph:two, root//graph:three]\n" == result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_somepath(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:uquery.bxl:somepath_test",
    )

    assert "[root//graph:one, root//graph:two, root//graph:three]\n" == result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_somepath_filtered(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:uquery.bxl:somepath_filtered_test",
    )

    assert (
        "[root//graph:one, root//graph:ten, root//graph:twenty]\n"
        + "[root//graph:one, root//graph:five, root//graph:six, root//graph:twenty]\n"
        == result.stdout
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_lazy_somepath(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:uquery.bxl:lazy_somepath_test",
    )

    assert "[root//graph:one, root//graph:two, root//graph:three]\n" == result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_lazy_somepath_filtered(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:uquery.bxl:lazy_somepath_filtered_test",
    )

    assert (
        "[root//graph:one, root//graph:ten, root//graph:twenty]\n"
        + "[root//graph:one, root//graph:five, root//graph:six, root//graph:twenty]\n"
        == result.stdout
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_kind(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:uquery.bxl:kind_test",
    )

    assert "foo" in result.stdout
    assert "bar" not in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_lazy_kind(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:uquery.bxl:lazy_kind_test",
    )

    assert "foo" in result.stdout
    assert "bar" not in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_inputs(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:uquery.bxl:inputs_test",
    )

    assert "TARGETS.fixture" in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_lazy_inputs(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:uquery.bxl:lazy_inputs_test",
    )

    assert "TARGETS.fixture" in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_filter(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:uquery.bxl:filter_test",
    )

    assert "root//bin:the_binary" in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_lazy_filter(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:uquery.bxl:lazy_filter_test",
    )

    assert "root//bin:the_binary" in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_attrregex_filter(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/uquery.bxl:attrregexfilter_test",
    )

    assert "foo" in result.stdout
    assert "bzzt" in result.stdout
    assert "bar" not in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_lazy_attrregex_filter(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/uquery.bxl:lazy_attrregexfilter_test",
    )

    assert "foo" in result.stdout
    assert "bzzt" in result.stdout
    assert "bar" not in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_attrfilter(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/uquery.bxl:attrfilter_test",
    )

    assert "foo" in result.stdout
    assert "bzzt" not in result.stdout
    assert "bar" not in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_lazy_attrfilter(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/uquery.bxl:lazy_attrfilter_test",
    )

    assert "foo" in result.stdout
    assert "bzzt" not in result.stdout
    assert "bar" not in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_owner(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/uquery.bxl:owner_test",
    )
    assert result.stdout == "[root//bin:the_binary]\n"

    result = await bsmr.bxl(
        "//bxl/uquery.bxl:owner_with_cell_path_test",
    )
    assert _replace_hash(result.stdout) == "[root//bin:the_binary]\n"


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_owner_list(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/uquery.bxl:owner_list_test",
    )
    assert (
        _replace_hash(result.stdout)
        == "[root//bin:the_binary, root//bin:the_binary_with_dir_srcs]\n"
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_lazy_owner(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/uquery.bxl:lazy_owner_test",
    )
    assert result.stdout == "[root//bin:the_binary]\n"

    result = await bsmr.bxl(
        "//bxl/uquery.bxl:lazy_owner_with_cell_path_test",
    )
    assert _replace_hash(result.stdout) == "[root//bin:the_binary]\n"


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_lazy_owner_list(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/uquery.bxl:lazy_owner_list_test",
    )
    assert (
        _replace_hash(result.stdout)
        == "[root//bin:the_binary, root//bin:the_binary_with_dir_srcs]\n"
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_targets_in_buildfile(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/uquery.bxl:targets_in_buildfile_test",
    )
    assert (
        result.stdout
        == "[root//bin:the_binary, root//bin:the_binary_with_dir_srcs, root//bin:platform]\n"
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_lazy_targets_in_buildfile(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/uquery.bxl:lazy_targets_in_buildfile_test",
    )
    assert (
        result.stdout
        == "[root//bin:the_binary, root//bin:the_binary_with_dir_srcs, root//bin:platform]\n"
    )


@bsmr_test(inplace=False, data_dir="bxl/simple", allow_soft_errors=True)
async def test_uquery_buildfile(bsmr: Bsmr) -> None:
    await bsmr.bxl("//bxl/uquery.bxl:buildfile_test")


@bsmr_test(inplace=False, data_dir="bxl/simple", allow_soft_errors=True)
async def test_uquery_lazy_buildfile(bsmr: Bsmr) -> None:
    await bsmr.bxl("//bxl/uquery.bxl:lazy_buildfile_test")


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_rdeps(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/uquery.bxl:rdeps_test",
    )
    assert result.stdout == "[root//bin:the_binary, root//lib:lib1, root//lib:file1]\n"


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_lazy_rdeps(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/uquery.bxl:lazy_rdeps_test",
    )
    assert result.stdout == "[root//bin:the_binary, root//lib:lib1, root//lib:file1]\n"


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_query_deps(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/uquery.bxl:deps_test",
    )
    assert (
        result.stdout
        == "[root//bin:the_binary, root//:data, root//lib:lib1, root//lib:lib2, root//lib:lib3, root//:foo_toolchain, root//:bin]\n"
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_lazy_deps(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/uquery.bxl:lazy_deps_test",
    )
    assert (
        result.stdout
        == "[root//bin:the_binary, root//:data, root//lib:lib1, root//lib:lib2, root//lib:lib3, root//:foo_toolchain, root//:bin]\n"
    )


@bsmr_test(inplace=False, data_dir="testsof")
async def test_uquery_testsof(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//uquery.bxl:testsof_test",
    )
    assert "root//:foo_test" in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_eval(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/uquery.bxl:eval_query_test",
    )
    assert result.stdout == "[root//bin/TARGETS.fixture]\n"

    result = await bsmr.bxl(
        "//bxl/uquery.bxl:eval_query_with_query_args",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_uquery_lazy_eval(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/uquery.bxl:lazy_eval_query_test",
    )
    assert result.stdout == "[root//bin/TARGETS.fixture]\n"

    result = await bsmr.bxl(
        "//bxl/uquery.bxl:lazy_eval_query_with_query_args",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_bxl_aquery_incompatible_targets(bsmr: Bsmr) -> None:
    # incompatible target should be skipped and the aquery should not fail
    result = await bsmr.bxl("//bxl/aquery.bxl:incompatible_targets")
    assert "Skipped 1 incompatible targets" in result.stderr
    assert "root//incompatible_targets:incompatible" in result.stderr


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_cquery_eval(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:cquery.bxl:eval_query_test",
    )

    assert "TARGETS.fixture" in result.stdout

    result = await bsmr.bxl(
        "//bxl/cquery.bxl:eval_query_with_query_args",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_cquery_allpaths(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:cquery.bxl:allpaths_test",
    )

    assert (
        "[root//graph:one (<unspecified>), root//graph:ten (<unspecified>), root//graph:eleven (<unspecified>), root//graph:two (<unspecified>), root//graph:three (<unspecified>)]\n"
        == result.stdout
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_cquery_somepath(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:cquery.bxl:somepath_test",
    )

    assert (
        "[root//graph:one (<unspecified>), root//graph:two (<unspecified>), root//graph:three (<unspecified>)]\n"
        == result.stdout
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_cquery_somepath_filtered(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:cquery.bxl:somepath_filtered_test",
    )

    assert (
        "[root//graph:one (<unspecified>), root//graph:ten (<unspecified>), root//graph:twenty (<unspecified>)]\n"
        + "[root//graph:one (<unspecified>), root//graph:five (<unspecified>), root//graph:six (<unspecified>), root//graph:twenty (<unspecified>)]\n"
        == result.stdout
    )


def random_string() -> str:
    return "".join(random.choice(string.ascii_lowercase) for i in range(256))

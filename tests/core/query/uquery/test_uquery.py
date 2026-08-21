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
from bsmr.tests.e2e_util.api.bsmr_result import BsmrResult
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.golden import golden

"""
If you need to add a directory that's isolated in bsmr/test/targets
(ex. some test of form @bsmr_test( data_dir=some_new_directory)),
then you will need to update isolated_targets in bsmr/test/targets/TARGETS.
Otherwise the test will fail because it cannot recognize the new directory.
"""


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


@bsmr_test(data_dir="bxl_simple")
async def test_uquery_none(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.uquery("""none"""),
        stderr_regex="Error parsing target pattern `none`",
    )

    await expect_failure(
        bsmr.uquery("""None"""),
        stderr_regex="expected value of type `targets`, got `None`:",
    )

    result = await bsmr.uquery(""":none""")
    assert result.stdout == "root//:none\n"

    result = await bsmr.uquery(""":None""")
    assert result.stdout == "root//:None\n"

    result = await bsmr.uquery("""':none'""")
    assert result.stdout == "root//:none\n"

    result = await bsmr.uquery("""':None'""")
    assert result.stdout == "root//:None\n"

    await expect_failure(
        bsmr.uquery("""set(none)"""),
        stderr_regex="Error parsing target pattern `none`",
    )

    await expect_failure(
        bsmr.uquery("""set(None)"""),
        # stderr_regex="expected value of type `targets`, got `None`:",
        stderr_regex="Error parsing target pattern `None`",
    )

    await expect_failure(
        bsmr.uquery("""set('none')"""),
        stderr_regex="Error parsing target pattern `none`",
    )

    await expect_failure(
        bsmr.uquery("""set('None')"""),
        stderr_regex="Error parsing target pattern `None`",
    )

    await expect_failure(
        bsmr.uquery("""filter('', none)"""),
        stderr_regex="Error parsing target pattern `none`",
    )

    await expect_failure(
        bsmr.uquery("""filter('', None)"""),
        stderr_regex=re.escape(
            "None is not a valid value for function `filter` argument [1] `set: *target or file expression*`"
        ),
    )

    result = await bsmr.uquery("""filter(none, :none)""")
    assert result.stdout == "root//:none\n"

    await expect_failure(
        bsmr.uquery("""filter(None, :None)"""),
        stderr_regex=re.escape(
            "None is not a valid value for function `filter` argument [0] `regex: *string*`"
        ),
    )

    result = await bsmr.uquery("""filter('none', :none)""")
    assert result.stdout == "root//:none\n"

    result = await bsmr.uquery("""filter('None', :None)""")
    assert result.stdout == "root//:None\n"

    result = await bsmr.uquery("""filter(none, ':none')""")
    assert result.stdout == "root//:none\n"

    await expect_failure(
        bsmr.uquery("""filter(None, ':None')"""),
        stderr_regex=re.escape(
            "None is not a valid value for function `filter` argument [0] `regex: *string*`"
        ),
    )

    result = await bsmr.uquery("""filter('none', ':none')""")
    assert result.stdout == "root//:none\n"

    result = await bsmr.uquery("""filter('None', ':None')""")
    assert result.stdout == "root//:None\n"

    await expect_failure(
        bsmr.uquery("""none()"""),
        stderr_regex="unknown function `none`:",
    )
    await expect_failure(
        bsmr.uquery("""None()"""),
        stderr_regex="in Eof",
    )


@bsmr_test(data_dir="bxl_simple")
async def test_uquery_inputs(bsmr: Bsmr) -> None:
    result = await bsmr.uquery("""inputs(set(root//bin:the_binary //lib:file1))""")
    assert result.stdout == "bin/TARGETS.fixture\n"

    result = await bsmr.uquery("""inputs(set())""")
    assert result.stdout == ""


@bsmr_test(data_dir="bxl_simple")
async def test_uquery_union(bsmr: Bsmr) -> None:
    result = await bsmr.uquery("""deps(root//lib:lib1) + set(root//data:data)""")
    assert result.stdout == "root//lib:file1\nroot//lib:lib1\nroot//data:data\n"

    result = await bsmr.uquery(
        """buildfile(root//bin:the_binary) + inputs(deps(root//lib:lib1))"""
    )
    assert result.stdout == "bin/TARGETS.fixture\nlib/TARGETS.fixture\n"

    result = await bsmr.uquery("""'root//bin:the_binary' + set(root//data:data)""")
    assert result.stdout == "root//bin:the_binary\nroot//data:data\n"


@bsmr_test(data_dir="bxl_simple")
async def test_uquery_owner(bsmr: Bsmr) -> None:
    result = await bsmr.uquery("""owner(bin/TARGETS.fixture)""")
    assert result.stdout == "root//bin:the_binary\n"

    result = await bsmr.uquery("""owner(data/bsmr/build/data.file)""")
    assert result.stdout == "root//data:data\n"

    # there's no buildfile in the root of the special bsmr, make sure that works
    result = await bsmr.uquery("""owner(special/file)""")
    assert "No owner" in result.stderr
    assert result.stdout == ""

    # there's a buildfile here, but no target owns the file
    result = await bsmr.uquery("""owner(.bsmr)""")
    assert "No owner" in result.stderr
    assert result.stdout == ""

    result = await bsmr.uquery(
        """owner(../data/bsmr/build/data.file)""", rel_cwd=Path("special")
    )
    assert result.stdout == "root//data:data\n"

    result = await bsmr.uquery("""owner(root//bin/TARGETS.fixture)""")
    assert result.stdout == "root//bin:the_binary\n"


@bsmr_test(data_dir="bxl_simple")
async def test_query_owner_with_explicit_package_boundary_violation(bsmr: Bsmr) -> None:
    result = await bsmr.uquery("""owner(package_boundary_violation/bin)""")
    assert "root//package_boundary_violation:bin" in result.stdout
    assert "root//:package_boundary_violation" in result.stdout


@bsmr_test(data_dir="bxl_simple", allow_soft_errors=True)
async def test_uquery_buildfile(bsmr: Bsmr) -> None:
    result = await bsmr.uquery("""buildfile(root//bin:the_binary)""")
    assert result.stdout == "bin/TARGETS.fixture\n"

    result = await bsmr.uquery("""buildfile(root//bin: + root//data:)""")
    assert result.stdout == "bin/TARGETS.fixture\ndata/TARGETS.fixture\n"

    result = await bsmr.uquery(
        """buildfile(owner(../data/bsmr/build/data.file))""", rel_cwd=Path("special")
    )
    assert result.stdout == "data/TARGETS.fixture\n"


@bsmr_test(data_dir="bxl_simple")
async def test_uquery_targets_in_buildfile(bsmr: Bsmr) -> None:
    result = await bsmr.uquery("""targets_in_buildfile(bin/TARGETS.fixture)""")
    assert (
        result.stdout
        == "\n".join(
            [
                "root//bin:setting",
                "root//bin:my_config",
                "root//bin:my_platform",
                "root//bin:the_binary",
                "root//bin:the_binary_with_dir_srcs",
                "root//bin:platform",
            ]
        )
        + "\n"
    )


@bsmr_test(data_dir="bxl_simple")
async def test_query_configuration_deps(bsmr: Bsmr) -> None:
    result = await bsmr.uquery(
        """deps(root//bin:the_binary, 1, configuration_deps())"""
    )
    assert "root//bin:my_config" in result.stdout


@bsmr_test(data_dir="bxl_simple")
async def test_deps(bsmr: Bsmr) -> None:
    result = await bsmr.uquery("""deps(root//bin:the_binary)""")
    assert (
        result.stdout
        == "\n".join(
            [
                "root//:foo_toolchain",
                "root//:bin",
                "root//lib:file3",
                "root//lib:lib3",
                "root//lib:file2",
                "root//lib:lib2",
                "root//lib:file1",
                "root//lib:lib1",
                "root//:genrule_binary",
                "root//:data",
                "root//bin:the_binary",
            ]
        )
        + "\n"
    )

    target_deps_expr = """deps(root//bin:the_binary, 100, target_deps())"""

    result = await bsmr.uquery(target_deps_expr)
    assert (
        result.stdout
        == "\n".join(
            [
                "root//bin:the_binary",
                "root//:data",
                "root//lib:lib1",
                "root//lib:lib2",
                "root//lib:lib3",
                "root//lib:file1",
                "root//lib:file2",
                "root//lib:file3",
            ]
        )
        + "\n"
    )

    # this is a little subtle, query's deps() function always forms a graph
    # with the nodes themselves so we subtract them out. It's not quite right
    # if a node in the graph of target deps were to have an exec dep on another.
    result = await bsmr.uquery(
        "deps({}, 1, exec_deps()) - {}".format(target_deps_expr, target_deps_expr)
    )
    assert (
        result.stdout
        == "\n".join(
            [
                "root//:foo_toolchain",
                "root//:bin",
                "root//:genrule_binary",
            ]
        )
        + "\n"
    )


@bsmr_test(data_dir="bxl_simple")
async def test_uquery_cell(bsmr: Bsmr) -> None:
    result = await bsmr.uquery("""//stuff:magic""", rel_cwd=Path("special"))
    assert result.stdout == "special//stuff:magic\n"


@bsmr_test(data_dir="bxl_simple")
async def test_uquery_relative(bsmr: Bsmr) -> None:
    result = await bsmr.uquery("""...""", rel_cwd=Path("special"))
    assert result.stdout == "special//stuff:magic\n"
    result = await bsmr.uquery("""...""", rel_cwd=Path("bin"))
    assert "root//bin:the_binary\n" in result.stdout


@bsmr_test(data_dir="bxl_simple")
async def test_uquery_provider_names(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.uquery("'root//bin:the_binary[provider_name]'"),
        stderr_regex="Expected a target pattern without providers",
    )

    await expect_failure(
        bsmr.uquery("'root//bin:the_binary#some_flavor'"),
        stderr_regex="Expected a target pattern without providers",
    )


@bsmr_test(data_dir="bxl_simple")
async def test_query_filter(bsmr: Bsmr) -> None:
    # Test uquery/cquery on target and file sets
    out = await bsmr.uquery("filter('the_binary$', root//...)")
    assert out.stdout == "root//bin:the_binary\n"
    out = await bsmr.cquery("filter('the_binary\\w', root//...)")
    assert (
        _replace_hash(out.stdout)
        == "root//bin:the_binary_with_dir_srcs (root//platforms:platform1#<HASH>)\n"
    )
    out = await bsmr.uquery("filter('fixture$', inputs(root//bin:the_binary))")
    assert out.stdout == "bin/TARGETS.fixture\n"
    out = await bsmr.cquery("filter('fixture$', inputs(root//bin:the_binary))")
    assert out.stdout == "bin/TARGETS.fixture\n"


@bsmr_test(setup_eden=True, data_dir="bxl_simple")
async def test_attributes(bsmr: Bsmr) -> None:
    out = await bsmr.uquery("set(root//bin:the_binary //lib:file1)")
    assert out.stdout == "root//bin:the_binary\nroot//lib:file1\n"

    json_out = await bsmr.uquery("--json", "set(root//bin:the_binary //lib:file1)")
    json_out = json.loads(json_out.stdout)
    assert ["root//bin:the_binary", "root//lib:file1"] == json_out

    attrs_out = await bsmr.uquery(
        "--output-attribute",
        "bsmr\\..*",
        "--output-attribute",
        "srcs",
        "--output-attribute",
        "deps",
        "set(root//bin:the_binary //lib:file1)",
    )
    attrs_json_out = await bsmr.uquery(
        "--output-attribute",
        "bsmr\\..*",
        "--output-attribute",
        "srcs",
        "--output-attribute",
        "deps",
        "--json",
        "set(root//bin:the_binary //lib:file1)",
    )
    # specifying any attrs enables json output
    assert attrs_json_out.stdout == attrs_out.stdout
    attrs_json_out = json.loads(attrs_json_out.stdout)
    assert {
        "root//bin:the_binary": {
            "bsmr.deps": [
                "root//:data",
                "root//lib:lib1",
                "root//lib:lib2",
                "root//lib:lib3",
                "root//:foo_toolchain",
                "root//:bin",
            ],
            "bsmr.package": "root//bin:TARGETS.fixture",
            "bsmr.tree_modifiers": ["cfg//os:linux"],
            "bsmr.type": "_foo_binary",
            "bsmr.configuration_deps": ["root//bin:my_platform", "root//bin:my_config"],
            "bsmr.oncall": None,
            "deps": ["root//lib:lib1", "root//lib:lib2", "root//lib:lib3"],
            "srcs": ["root//bin/TARGETS.fixture"],
        },
        "root//lib:file1": {
            "bsmr.deps": [],
            "bsmr.package": "root//lib:TARGETS.fixture",
            "bsmr.tree_modifiers": ["cfg//os:linux"],
            "bsmr.type": "_foo_genrule",
            "bsmr.configuration_deps": ["root//platforms:platform1"],
            "bsmr.oncall": None,
        },
    } == attrs_json_out


@bsmr_test(data_dir="bxl_simple")
async def test_dot(bsmr: Bsmr) -> None:
    out = await bsmr.uquery("--dot", "deps(root//bin:the_binary, 100, target_deps())")
    golden(output=out.stdout, rel_path="bxl_simple/expected/dot/deps.golden")

    out = await bsmr.uquery(
        "--dot",
        "--output-attribute=name",
        "--output-attribute=^deps",
        "--output-attribute=cmd",
        "deps(root//bin:the_binary, 100, target_deps()) - //platforms:",
    )
    golden(output=out.stdout, rel_path="bxl_simple/expected/dot/attrs.golden")

    out = await bsmr.uquery(
        "--dot",
        "deps(root//bin:the_binary, 100, target_deps()) - set(//lib: //platforms:)",
    )
    golden(output=out.stdout, rel_path="bxl_simple/expected/dot/subgraph.golden")


@bsmr_test(data_dir="bxl_simple")
async def test_dot_compact(bsmr: Bsmr) -> None:
    out = await bsmr.uquery(
        "--dot-compact", "deps(root//bin:the_binary, 100, target_deps())"
    )
    golden(
        output=out.stdout,
        rel_path="bxl_simple/expected/dot_compact/deps.golden",
    )

    out = await bsmr.uquery(
        "--dot-compact",
        "--output-attribute=name",
        "--output-attribute=^deps",
        "--output-attribute=cmd",
        "deps(root//bin:the_binary, 100, target_deps()) - //platforms:",
    )
    golden(
        output=out.stdout,
        rel_path="bxl_simple/expected/dot_compact/attrs.golden",
    )

    out = await bsmr.uquery(
        "--dot-compact",
        "deps(root//bin:the_binary, 100, target_deps()) - set(//lib: //platforms:)",
    )
    golden(
        output=out.stdout,
        rel_path="bxl_simple/expected/dot_compact/subgraph.golden",
    )


# Tests for "%Ss" uses
@bsmr_test(data_dir="bxl_simple")
async def test_args_as_set(bsmr: Bsmr) -> None:
    out = await bsmr.uquery("%Ss", "root//bin:the_binary", "//lib:file1")
    assert out.stdout == "root//bin:the_binary\nroot//lib:file1\n"

    result = await bsmr.uquery("--json", "%Ss", "root//bin:the_binary", "//lib:file1")
    json_out = json.loads(result.stdout)
    assert json_out == ["root//bin:the_binary", "root//lib:file1"]


@bsmr_test(data_dir="bxl_simple")
async def test_multi_uquery(bsmr: Bsmr) -> None:
    out = await bsmr.uquery("%s", "root//bin:the_binary", "//lib:file1")
    assert out.stdout == "root//bin:the_binary\nroot//lib:file1\n"

    result = await bsmr.uquery(
        "owner(%s)", "bin/TARGETS.fixture", "data/bsmr/build/data.file"
    )
    assert result.stdout == "root//bin:the_binary\nroot//data:data\n"

    result = await bsmr.uquery(
        "--json", "owner(%s)", "bin/TARGETS.fixture", "data/bsmr/build/data.file"
    )
    json_out = json.loads(result.stdout)

    assert json_out == {
        "bin/TARGETS.fixture": ["root//bin:the_binary"],
        "data/bsmr/build/data.file": ["root//data:data"],
    }

    # match legacy's strange handling of multi-query with --output-attribute
    result = await bsmr.uquery(
        "--json",
        "--output-attribute=name",
        "owner(%s)",
        "bin/TARGETS.fixture",
        "data/bsmr/build/data.file",
    )
    json_out = json.loads(result.stdout)

    assert json_out == {
        "root//bin:the_binary": {"name": "the_binary"},
        "root//data:data": {"name": "data"},
    }

    # test a case where the query for one arg fails. The process should exit with a non-zero code, but
    # the produced output should be valid json with an appropriate error indicator.
    failure = await expect_failure(
        bsmr.uquery("--json", "inputs(%s)", "//data:data", "xyz")
    )
    json_out = json.loads(failure.stdout)
    assert "$error" in json_out["xyz"]
    assert json_out["//data:data"] == ["data/bsmr/build/data.file"]

    # Test where the parameter is not a literal, but a query fragment
    out = await bsmr.uquery("%s", "deps(root//lib:lib1)")
    assert out.stdout == "root//lib:file1\nroot//lib:lib1\n"

    out = await bsmr.uquery("owner(%s)", "inputs(root//bin:the_binary)")
    assert out.stdout == "root//bin:the_binary\n"

    out = await bsmr.uquery("owner(%s)", "data/bsmr/build/data.file")
    assert out.stdout == "root//data:data\n"

    # We'd really prefer this to be an error, but Legacy allows it
    out = await bsmr.uquery("owner(%s", "data/bsmr/build/data.file)")
    assert out.stdout == "root//data:data\n"


@bsmr_test(data_dir="testsof")
async def test_testsof(bsmr: Bsmr) -> None:
    out = await bsmr.uquery("testsof(//:foo_lib)")

    assert "root//:foo_test" in out.stdout
    assert "root//:foo_extra_test" in out.stdout
    assert "root//:foo_lib" not in out.stdout


@bsmr_test(data_dir="directory_sources")
async def test_directory_source(bsmr: Bsmr) -> None:
    await bsmr.build(":a_file")
    await bsmr.build(":a_dir")

    result = await bsmr.query("owner(dir/file1.txt)")
    assert result.stdout == "root//:a_dir\n"
    result = await bsmr.query("inputs(:a_dir)")
    assert (
        result.stdout == "dir/file1.txt\ndir/subdir/file2.txt\ndir/subdir/file3.txt\n"
    )

    # Can't reference files that don't exist
    await expect_failure(
        bsmr.build("does_not_exist:"),
        stderr_regex="Source file `does_not_exist` does not exist as a member of package",
    )

    # Want to make sure we can't do a package boundary violation
    # Currently these are soft errors
    await expect_failure(
        bsmr.build("subpackage:"),
        stderr_regex="Source file `subpackage` does not exist as a member of package",
    )

    await expect_failure(
        bsmr.build("dir_with_subpackage"),
        stderr_regex="may not cover any subpackages, but includes subpackage `dir_with_subpackage/subpackage`.",
    )


@bsmr_test(data_dir="oncall")
async def test_oncall(bsmr: Bsmr) -> None:
    out = await bsmr.uquery("//:foo", "--output-attribute=oncall")
    assert '"magic"' in out.stdout
    out = await bsmr.cquery("//:bar", "--output-attribute=oncall")
    assert '"magic"' in out.stdout


@bsmr_test(data_dir="oncall")
async def test_output_all_attributes(bsmr: Bsmr) -> None:
    def contains(out: BsmrResult, want: list[str], notwant: list[str]) -> None:
        x = json.loads(out.stdout)["root//:foo"]
        for w in want:
            assert w in x
        for w in notwant:
            assert w not in x

    out = await bsmr.uquery("//:foo", "--output-all-attributes", "--json")
    contains(
        out,
        [
            "bsmr.type",
            "name",
            "bsmr.oncall",
            "bsmr.package",
            "bsmr.configuration_deps",
            "bsmr.deps",
            "visibility",
        ],
        ["madeup"],
    )
    out = await bsmr.uquery("//:foo", "--output-basic-attributes", "--json")
    contains(
        out,
        ["bsmr.type", "name", "bsmr.package", "visibility"],
        ["bsmr.oncall", "bsmr.configuration_deps"],
    )


@bsmr_test(data_dir="bxl_simple")
async def test_output_format_starlark_golden(bsmr: Bsmr) -> None:
    result = await bsmr.uquery(
        "--output-format=starlark",
        "--stack",
        "//lib:",
    )

    golden(
        output=result.stdout,
        rel_path="output_starlark.golden.out",
    )


@bsmr_test(data_dir="bxl_simple")
async def test_uquery_rdeps(bsmr: Bsmr) -> None:
    result = await bsmr.query("""rdeps(root//bin:the_binary, //lib:file1)""")
    assert result.stdout == "root//bin:the_binary\nroot//lib:lib1\nroot//lib:file1\n"

    result = await bsmr.query("""rdeps(root//bin:the_binary, //lib:file1, 0)""")
    assert result.stdout == "root//lib:file1\n"

    result = await bsmr.query("""rdeps(root//bin:the_binary, //lib:file1, 1)""")
    assert result.stdout == "root//lib:lib1\nroot//lib:file1\n"

    result = await bsmr.query("""rdeps(root//bin:the_binary, //lib:file1, 100)""")
    assert result.stdout == "root//bin:the_binary\nroot//lib:lib1\nroot//lib:file1\n"


@bsmr_test(data_dir="bxl_simple")
async def test_query_attrfilter_special_attribute(bsmr: Bsmr) -> None:
    out = await bsmr.uquery(
        "attrfilter(bsmr.package, 'root//bin:TARGETS.fixture',root//bin:the_binary)"
    )
    assert out.stdout.strip() == "root//bin:the_binary"


# Tests for intersect and except operators on FileSet, TargetSet, and String types
# These tests verify the fix for https://github.com/facebook/buck2/issues/1109
@bsmr_test(data_dir="set_operators")
async def test_uquery_fileset_intersect(bsmr: Bsmr) -> None:
    """Test FileSet intersect FileSet using inputs()."""
    result = await bsmr.uquery(
        """inputs(root//:lib_a) intersect inputs(root//:lib_b)"""
    )
    assert result.stdout == "common.txt\n"


@bsmr_test(data_dir="set_operators")
async def test_uquery_fileset_except(bsmr: Bsmr) -> None:
    """Test FileSet except FileSet using inputs()."""
    result = await bsmr.uquery("""inputs(root//:lib_a) except inputs(root//:lib_b)""")
    assert result.stdout == "lib_a.txt\n"


@bsmr_test(data_dir="set_operators")
async def test_uquery_fileset_intersect_string(bsmr: Bsmr) -> None:
    """Test FileSet intersect String."""
    result = await bsmr.uquery("""inputs(root//:lib_a) intersect "common.txt" """)
    assert result.stdout == "common.txt\n"


@bsmr_test(data_dir="set_operators")
async def test_uquery_fileset_except_string(bsmr: Bsmr) -> None:
    """Test FileSet except String."""
    result = await bsmr.uquery("""inputs(root//:lib_a) except "common.txt" """)
    assert result.stdout == "lib_a.txt\n"


@bsmr_test(data_dir="set_operators")
async def test_uquery_string_intersect_fileset(bsmr: Bsmr) -> None:
    """Test String intersect FileSet."""
    result = await bsmr.uquery(""" "common.txt" intersect inputs(root//:lib_a)""")
    assert result.stdout == "common.txt\n"


@bsmr_test(data_dir="set_operators")
async def test_uquery_string_except_fileset(bsmr: Bsmr) -> None:
    """Test String except FileSet (string not in fileset)."""
    result = await bsmr.uquery(""" "lib_a.txt" except inputs(root//:lib_b)""")
    assert result.stdout == "lib_a.txt\n"


@bsmr_test(data_dir="set_operators")
async def test_uquery_targetset_intersect(bsmr: Bsmr) -> None:
    """Test TargetSet intersect TargetSet using set()."""
    result = await bsmr.uquery(
        """set(root//:lib_a root//:app) intersect set(root//:lib_b root//:app)"""
    )
    assert result.stdout == "root//:app\n"


@bsmr_test(data_dir="set_operators")
async def test_uquery_targetset_except(bsmr: Bsmr) -> None:
    """Test TargetSet except TargetSet using set()."""
    result = await bsmr.uquery(
        """set(root//:lib_a root//:app) except set(root//:app)"""
    )
    assert result.stdout == "root//:lib_a\n"


@bsmr_test(data_dir="set_operators")
async def test_uquery_targetset_intersect_string(bsmr: Bsmr) -> None:
    """Test TargetSet intersect String."""
    result = await bsmr.uquery(
        """set(root//:lib_a root//:app) intersect "root//:lib_a" """
    )
    assert result.stdout == "root//:lib_a\n"


@bsmr_test(data_dir="set_operators")
async def test_uquery_targetset_except_string(bsmr: Bsmr) -> None:
    """Test TargetSet except String."""
    result = await bsmr.uquery("""set(root//:lib_a root//:app) except "root//:app" """)
    assert result.stdout == "root//:lib_a\n"


@bsmr_test(data_dir="set_operators")
async def test_uquery_string_intersect_targetset(bsmr: Bsmr) -> None:
    """Test String intersect TargetSet."""
    result = await bsmr.uquery(
        """ "root//:lib_a" intersect set(root//:lib_a root//:app)"""
    )
    assert result.stdout == "root//:lib_a\n"


@bsmr_test(data_dir="set_operators")
async def test_uquery_string_except_targetset(bsmr: Bsmr) -> None:
    """Test String except TargetSet (string not in targetset)."""
    result = await bsmr.uquery(
        """ "root//:app" except set(root//:lib_a root//:lib_b)"""
    )
    assert result.stdout == "root//:app\n"


@bsmr_test(data_dir="set_operators")
async def test_uquery_string_intersect_string(bsmr: Bsmr) -> None:
    """Test String intersect String for targets."""
    result = await bsmr.uquery(""" "root//:lib_a" intersect "root//:lib_a" """)
    assert result.stdout == "root//:lib_a\n"


@bsmr_test(data_dir="set_operators")
async def test_uquery_string_except_string(bsmr: Bsmr) -> None:
    """Test String except String (different targets)."""
    result = await bsmr.uquery(""" "root//:app" except "root//:lib_a" """)
    assert result.stdout == "root//:app\n"

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
import os
import platform
import random
import string
import subprocess
from pathlib import Path

import pytest
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.api.bsmr_result import BsmrException
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.utils import (
    get_bsmr_re_use_case,
    json_get,
    read_what_ran,
)


@bsmr_test(data_dir="anon_exec_deps")
async def test_anon_target_exec_deps(bsmr: Bsmr) -> None:
    await bsmr.build("//tests:exec_dep_good", "--remote-only")

    await expect_failure(
        bsmr.build("//tests:exec_dep_bad", "--local-only"),
        stderr_regex="Exec deps and the current anon target must have the same execution platform resolution",
    )

    await expect_failure(
        bsmr.build("//tests:exec_dep_rejects_dep"),
        stderr_regex="exec dep is missing the execution platform resolution",
    )


@bsmr_test(data_dir="args")
async def test_args(bsmr: Bsmr) -> None:
    result = await bsmr.build("//:bin")
    output = result.get_build_report().output_for_target("//:bin")
    assert (
        output.read_text().rstrip()
        == "<foo_compiler> -- <foo_compiler_flags>\nlib1 -- this is lib1\nlib2 -- this is lib2"
    )


@bsmr_test(data_dir="prelude_import")
async def test_prelude_imported_once(bsmr: Bsmr) -> None:
    # See the comments in the relevant targets files: they explain how this
    # test works.
    await bsmr.build("cell1//...", "cell2//...")


def read_all_outputs(bsmr: Bsmr, report: str) -> list[str]:
    ret = []

    with open(bsmr.cwd / report) as f:
        report = json.load(f)
        for _target, state in report["results"].items():
            ret.extend(state["outputs"].get("DEFAULT", []))
            ret.extend(state["other_outputs"].get("DEFAULT", []))

    return ret


@bsmr_test(data_dir="build_providers")
async def test_build_providers(bsmr: Bsmr) -> None:
    await bsmr.build(
        "//:target",
        "--build-default-info",
        "--skip-run-info",
        "--skip-test-info",
        "--build-report",
        "report",
    )

    outputs = read_all_outputs(bsmr, "report")
    assert any("/build" in o for o in outputs)
    assert all("/run" not in o for o in outputs)
    assert all("/test" not in o for o in outputs)

    await bsmr.build(
        "//:target",
        "--skip-default-info",
        "--build-run-info",
        "--skip-test-info",
        "--build-report",
        "report",
    )

    outputs = read_all_outputs(bsmr, "report")
    assert all("/build" not in o for o in outputs)
    assert all("/test" not in o for o in outputs)

    await bsmr.build(
        "//:target",
        "--skip-default-info",
        "--skip-run-info",
        "--build-test-info",
        "--build-report",
        "report",
    )

    outputs = read_all_outputs(bsmr, "report")
    assert all("/build" not in o for o in outputs)
    assert all("/run" not in o for o in outputs)


@bsmr_test(data_dir="projected_artifacts")
@pytest.mark.parametrize(
    "target",
    [
        # Check building the whole thing
        "//...",
        # Check building just one target, which may reveal bugs if things are
        # materialized differently when a projected target uses them.
        "//:check_c_b_local",
    ],
)
async def test_projected_artifacts(bsmr: Bsmr, target: str) -> None:
    await bsmr.build(target)


@bsmr_test(data_dir="upload_all_actions")
async def test_upload_all_actions(bsmr: Bsmr) -> None:
    with open(bsmr.cwd / "src", "w") as src:
        src.write(random_string())

    # This action includes `src` and is forced to run locally. This means RE
    # can never have seen it (and we'll check that later by asserting there is
    # only 1 cache query, excluding local actions).
    await bsmr.build("//:cp", "--upload-all-actions")

    what_ran = await read_what_ran(
        bsmr, "--emit-cache-queries", "--skip-local-executions"
    )
    assert len(what_ran) == 1

    # Now, download the action. This will succeed only if we uploaded it.
    digest = what_ran[0]["reproducer"]["details"]["digest"]
    use_case = await get_bsmr_re_use_case(bsmr)
    subprocess.check_call(
        [
            "dotslash",
            os.environ["RECLI"],
            "--use-case",
            use_case,
            "cas",
            "download-action",
            digest,
        ]
    )


@bsmr_test(data_dir="unified_root")
async def test_unified_root(bsmr: Bsmr) -> None:
    # The nearest .bsmr must shield the invocation from an invalid parent config.
    await bsmr.build(":inner", rel_cwd=Path("rooted/cell"))


@bsmr_test(data_dir="cell_delete")
async def test_cell_deletion(bsmr: Bsmr) -> None:
    """
    This is a regression test for https://github.com/facebook/buck2/pull/43,
    including the similar issue with directories that was fixed first.
    """
    await bsmr.targets(":")
    (bsmr.cwd / "hello").mkdir()
    await bsmr.targets(":")
    (bsmr.cwd / "hello").rmdir()
    await bsmr.targets(":")


@bsmr_test(
    data_dir="invalid_file_invalidation",
    skip_for_os=["windows"],
    setup_eden=True,
)
async def test_invalid_file_invalidation(bsmr: Bsmr) -> None:
    """
    This is a regression test for T136963408.
    """

    await bsmr.build(":root")

    src = bsmr.cwd / "src"
    invalid = src / "\\"
    invalid_nested = src / "\\" / "a"
    invalid_nested_invalid = src / "\\" / "\\"

    # Create an invalid file. Build should work.
    invalid.touch()
    output = await bsmr.build(":root")
    assert "is not valid. Add the path to" in output.stderr

    # Delete it, build should work.
    invalid.unlink()
    await bsmr.build(":root")

    # Create an invalid dir. Build should still work.
    invalid_nested.mkdir(parents=True)
    output = await bsmr.build(":root")
    assert "is not valid. Add the path to" in output.stderr

    # And delete it. Things should work.
    invalid_nested.rmdir()
    invalid.rmdir()
    await bsmr.build(":root")

    # Finally, do an invalid file inside an invalid dir...
    invalid_nested_invalid.mkdir(parents=True)
    output = await bsmr.build(":root")
    assert "is not valid. Add the path to" in output.stderr

    # And delete it. Things should again.
    invalid_nested_invalid.rmdir()
    invalid.rmdir()
    await bsmr.build(":root")


@bsmr_test(data_dir="concurrency")
async def test_concurrency(bsmr: Bsmr) -> None:
    await bsmr.build("//:weight", "--local-only", "--no-remote-cache")

    # Now, since our commands request 20% of resources, check that a no point
    # we had more than 5 running commands. Also check that we found the right
    # amount of commands.
    log = (await bsmr.log("show")).stdout.strip().splitlines()

    running_execs = {}
    execs_done = 0

    for line in log:
        id = json_get(line, "Event", "span_id")

        is_end = json_get(
            line,
            "Event",
            "data",
            "SpanEnd",
        )

        if is_end:
            if running_execs.pop(id, None) is not None:
                execs_done += 1

            continue

        is_local_exec = json_get(
            line,
            "Event",
            "data",
            "SpanStart",
            "data",
            "ExecutorStage",
            "stage",
            "Local",
            "stage",
            "Execute",
        )

        if is_local_exec:
            running_execs[id] = True

        # Check that concurrently running local commands
        # don't exceed 5.
        assert len(running_execs) <= 5

    assert execs_done == 10


@bsmr_test(data_dir="fail_fast")
async def test_fail_fast(bsmr: Bsmr) -> None:
    with pytest.raises(BsmrException) as exc:
        await bsmr.build(
            "root//:mixed",
            "root//:slow",
            "--local-only",
            "--no-remote-cache",
        )

    assert "fast_default_output" in exc.value.stderr
    assert "slow_default_output" in exc.value.stderr
    assert "slow_other_output" in exc.value.stderr

    with pytest.raises(BsmrException) as exc:
        await bsmr.build(
            "root//:mixed",
            "root//:slow",
            "--local-only",
            "--no-remote-cache",
            "--fail-fast",
        )

    assert "fast_default_output" in exc.value.stderr
    assert "slow_default_output" not in exc.value.stderr
    assert "slow_other_output" not in exc.value.stderr


@bsmr_test(data_dir="keep_going_build")
async def test_keep_going(bsmr: Bsmr) -> None:
    with pytest.raises(BsmrException) as exc:
        await bsmr.build(
            "root//:top",
            "--local-only",
            "--no-remote-cache",
        )

    assert "fast_action" in exc.value.stderr
    assert "slow_action" not in exc.value.stderr

    # Dont want to re-attach to the ongoing evaluation for slow_action.
    # Normally that gets cancelled, but even so that's still a race.
    await bsmr.kill()

    with pytest.raises(BsmrException) as exc:
        await bsmr.build(
            "root//:top", "--local-only", "--no-remote-cache", "--keep-going"
        )

    assert "fast_action" in exc.value.stderr
    assert "slow_action" in exc.value.stderr


@bsmr_test(data_dir="cleanup")
async def test_cleanup(bsmr: Bsmr) -> None:
    # Test for T85589819 - broken cleanup
    target_pattern = "//:cleanup"
    result = await bsmr.build(target_pattern)
    output = result.get_build_report().output_for_target(target_pattern)

    # The output should be something like path/__cleanup__/out/dir1/dir2/output.txt
    # We want to ensure that if we make a file dir1 or dir1/dir2, cleanup still works
    output.unlink()
    output.parent.rmdir()
    output.parent.write_text("File that must be deleted")
    await bsmr.kill()
    await bsmr.build(target_pattern)

    output.unlink()
    output.parent.rmdir()
    output.parent.parent.rmdir()
    output.parent.parent.write_text("File that must be deleted")
    await bsmr.build(target_pattern)


@bsmr_test(data_dir="log_action_keys")
async def test_log_action_keys(bsmr: Bsmr) -> None:
    async def read_action_keys() -> list[tuple[str, str]]:
        out = await read_what_ran(bsmr)
        return [
            (
                line["reproducer"]["executor"],
                line["reproducer"]["details"]["action_key"],
            )
            for line in out
        ]

    seed = random_string()
    action_key = "executor root//:test (<unspecified>) touch"

    # Run on RE
    await bsmr.build(
        ":test", "-c", f"test.seed={seed}", "-c", "bsmr.log_action_keys=true"
    )
    assert await read_action_keys() == [("Re", action_key)]

    await bsmr.kill()

    # Run on RE again, get a cache hit this time
    await bsmr.build(
        ":test", "-c", f"test.seed={seed}", "-c", "bsmr.log_action_keys=true"
    )

    assert await read_action_keys() == [("Cache", action_key)]


@bsmr_test(data_dir="roots")
async def test_roots(bsmr: Bsmr) -> None:
    res = await bsmr.build("root//:test", "other//:test")

    is_windows: bool = platform.system() == "Windows"

    def platformify(path: str) -> str:
        if is_windows:
            return path.replace("/", "\\")
        return path

    output = res.get_build_report().output_for_target("root//:test")
    with open(output) as f:
        j = json.load(f)
        print(j)
        assert (bsmr.cwd / j["fixture_relative_to_cell"]).exists()
        assert (bsmr.cwd / j["fixture_relative_to_project"]).exists()

        assert j["cell_relative_to_fixture"] == platformify("../../../../../../..")
        assert j["project_relative_to_fixture"] == platformify("../../../../../../..")

    output = res.get_build_report().output_for_target("other//:test")
    with open(output) as f:
        j = json.load(f)
        assert (bsmr.cwd / "other" / j["fixture_relative_to_cell"]).exists()
        assert (bsmr.cwd / j["fixture_relative_to_project"]).exists()

        assert j["cell_relative_to_fixture"] == platformify(
            "../../../../../../../other"
        )
        assert j["project_relative_to_fixture"] == platformify("../../../../../../..")


@bsmr_test(data_dir="tmpdir")
async def test_tmpdir(bsmr: Bsmr) -> None:
    await bsmr.build("root//:")


def random_string() -> str:
    return "".join(random.choice(string.ascii_lowercase) for i in range(256))


@bsmr_test(data_dir="artifact_consistency")
async def test_artifact_consistency(bsmr: Bsmr) -> None:
    out = await bsmr.build_without_report(
        ":gen[file3]",
        "--local-only",
        "--out=-",
    )

    assert out.stdout == "This is file3"

    out = await bsmr.build_without_report(
        "-c",
        "gen.idx=2",
        ":gen[file3]",
        "--local-only",
        "--out=-",
    )
    assert out.stdout == "This is file3"

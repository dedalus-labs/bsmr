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


from __future__ import annotations

import json
import os
import re
import subprocess
from pathlib import Path
from tempfile import NamedTemporaryFile, TemporaryDirectory

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test, env


def assert_path_in_manifest(path: str, manifest_paths: list[str]) -> None:
    assert path in manifest_paths, f"expected manifest to contain {path}"


def assert_link_in(
    needle: dict[str, str | None], haystack: list[dict[str, str | None]]
) -> None:
    assert needle in haystack, (
        f"expected haystack to contain link: {needle['link']} --> {needle['target']}"
    )


def assert_path_exists(path: str) -> None:
    assert os.path.exists(path), f"expected {path} to exist"


def assert_output_paths_materialized(bsmr_cwd: Path, paths: list[str]) -> None:
    for path in paths:
        if re.match(r"bsmr-out\/.+\/{art,offline-cache}/.+\/.+\/.+", path) is not None:
            assert_path_exists(os.path.join(bsmr_cwd, path))


def hg_init(cwd: Path) -> None:
    subprocess.run(["hg", "init"], check=True, cwd=cwd)
    hg_config_reponame(cwd)


def hg_config_reponame(cwd: Path) -> None:
    subprocess.run(
        ["hg", "config", "remotefilelog.reponame", "--local", "no-repo"],
        check=True,
        cwd=cwd,
    )


def _setup_bsmrconfig_digest_algorithms(bsmr: Bsmr) -> None:
    # The digests in `//cas_artifact:` require the bsmrconfig.
    with open(bsmr.cwd / ".bsmr", "a") as bsmrconfig:
        bsmrconfig.write("[bsmr]\n")
        bsmrconfig.write("digest_algorithms = BLAKE3-KEYED,SHA1\n")


# Tracing I/O not implemented for Windows.
@bsmr_test(skip_for_os=["windows"])
async def test_simple_binary_build(bsmr: Bsmr) -> None:
    # Since this is an inplace test, we need to fake an hg repo so that export-manifest
    # can extract the repo revision.
    hg_init(cwd=bsmr.cwd)

    await bsmr.debug("trace-io", "enable")
    await bsmr.build("root//hello_world:welcome")
    out = await bsmr.debug("trace-io", "export-manifest")
    manifest = json.loads(out.stdout)

    assert (
        manifest["repository"]["revision"] == "0000000000000000000000000000000000000000"
    ), "expected manifest to be at null revision"
    assert manifest["repository"]["name"] == "no-repo", (
        "expected repo name to be no-repo"
    )

    assert_path_in_manifest("hello_world/main.cpp", manifest["paths"])


@bsmr_test(skip_for_os=["windows"])
async def test_external_bsmrconfig_path_included_in_manifest(bsmr: Bsmr) -> None:
    hg_init(cwd=bsmr.cwd)

    with NamedTemporaryFile("w") as tmp:
        tmpname = tmp.name
        tmp.writelines(
            [
                "[bsmr]",
                "  foo = bar",
            ]
        )

        await bsmr.debug("trace-io", "enable")
        await bsmr.build("root//hello_world:welcome", "--config-file", tmpname)
        out = await bsmr.debug("trace-io", "export-manifest")

    manifest = json.loads(out.stdout)

    assert_path_in_manifest(str(Path(tmpname).resolve()), manifest["external_paths"])


# More complicated example with binary depending on multiple libraries.
@bsmr_test(skip_for_os=["windows"])
async def test_binary_with_deps(bsmr: Bsmr) -> None:
    hg_init(cwd=bsmr.cwd)

    await bsmr.debug("trace-io", "enable")
    await bsmr.build("root//linking:root")
    out = await bsmr.debug("trace-io", "export-manifest")
    manifest = json.loads(out.stdout)

    assert (
        manifest["repository"]["revision"] == "0000000000000000000000000000000000000000"
    ), "expected manifest to be at null revision"
    assert manifest["repository"]["name"] == "no-repo", (
        "expected repo name to be no-repo"
    )

    assert_path_in_manifest("linking/main.cpp", manifest["paths"])
    assert_path_in_manifest("linking/static.cpp", manifest["paths"])
    assert_path_in_manifest("linking/static.h", manifest["paths"])
    assert_path_in_manifest("linking/shared.h", manifest["paths"])


# Multiple builds should be logical union of all input files of all builds.
@bsmr_test(skip_for_os=["windows"])
async def test_multiple_builds(bsmr: Bsmr) -> None:
    hg_init(cwd=bsmr.cwd)

    await bsmr.debug("trace-io", "enable")
    await bsmr.build("root//linking:root")
    await bsmr.build("root//hello_world:welcome")
    out = await bsmr.debug("trace-io", "export-manifest")
    manifest = json.loads(out.stdout)

    # From first build
    assert_path_in_manifest("linking/shared.h", manifest["paths"])
    # From second build
    assert_path_in_manifest("hello_world/main.cpp", manifest["paths"])


# Symlinks should show up in the *_symlinks attributes of the manifest.
@bsmr_test(setup_eden=True, skip_for_os=["windows"])
@env("BSMR_HARD_ERROR", "false")
async def test_symlinks(bsmr: Bsmr) -> None:
    hg_config_reponame(cwd=bsmr.cwd)

    def symlink(link: str, target: str) -> None:
        """
        Symlinks link --> target. Assumes we're based in the bsmr cwd, so link must be relative.
        """
        os.symlink(target, os.path.join(bsmr.cwd, link))

    # Set up symlinks during the test; bsmr will read everything behind symlinks while
    # setting up for the test otherwise.
    # Symlinks for root//symlinks:relative_link
    symlink("symlinks/main.cpp", "../hello_world/main.cpp")

    # Symlinks for root//symlinks:external_link
    with TemporaryDirectory() as tempdir:
        t = Path(tempdir)
        absolute_target = t / "include" / "clang" / "Basic" / "Visibility.h"
        absolute_target.parent.mkdir(parents=True)
        absolute_target.touch()

        traverses_symlink = t / "include" / "llvm" / "PassRegistry.h"
        traverses_symlink.parent.mkdir(parents=True)
        traverses_symlink.touch()

        symlink("symlinks/PassRegistry.h", str(absolute_target))
        symlink("symlinks/include", str(t / "include"))

        await bsmr.debug("trace-io", "enable")
        await bsmr.build("root//symlinks:relative_link")
        await bsmr.build("root//symlinks:external_link")

    out = await bsmr.debug("trace-io", "export-manifest")
    manifest = json.loads(out.stdout)

    assert_link_in(
        {"link": "symlinks/main.cpp", "target": "hello_world/main.cpp"},
        manifest["relative_symlinks"],
    )
    assert_link_in(
        {
            "link": "symlinks/PassRegistry.h",
            "target": str(absolute_target),
            "remaining_path": None,
        },
        manifest["external_symlinks"],
    )
    assert_link_in(
        {
            "link": "symlinks/include",
            "target": str(t / "include"),
            "remaining_path": "clang/Basic/Visibility.h",
        },
        manifest["external_symlinks"],
    )
    assert_path_in_manifest("symlinks/other.cpp", manifest["paths"])


# Validate that manifest includes downloaded http_archive path in bsmr-out.
@bsmr_test(skip_for_os=["windows"])
async def test_includes_http_archive_in_manifest(bsmr: Bsmr) -> None:
    hg_init(cwd=bsmr.cwd)

    await bsmr.debug("trace-io", "enable")
    await bsmr.build("root//http_archive:test_zip")
    out = await bsmr.debug("trace-io", "export-manifest")
    manifest = json.loads(out.stdout)

    assert any(
        re.match(
            r"bsmr-out/.+/offline-cache/.+/http_archive/__test_zip__/download", path
        )
        for path in manifest["paths"]
    ), "manifest should contain http_archive cached output"
    assert_output_paths_materialized(bsmr.cwd, manifest["paths"])


# Ensure offline-cache bsmr-out dir is _not_ created when not doing I/O tracing.
@bsmr_test(skip_for_os=["windows"])
async def test_no_tracing_does_not_write_offline_cache_for_http_archive(
    bsmr: Bsmr,
) -> None:
    await bsmr.build("root//http_archive:test_zip")
    assert not os.path.exists(os.path.join(bsmr.cwd, "bsmr-out/offline-cache")), (
        "offline cache should not exist when not doing I/O tracing"
    )


# Validate that when bsmrconfig use_network_action_output_cache=true is set we use the
# offline-cache action output instead of fetching from the network.
@bsmr_test(
    skip_for_os=["windows"],
    extra_bsmr_config={"bsmr": {"sqlite_materializer_state": "false"}},
)
@env("BSMR_LOG", "bsmr_execute_impl::materializers=trace")
async def test_fake_offline_http_archive_uses_offline_cache(bsmr: Bsmr) -> None:
    hg_init(cwd=bsmr.cwd)

    # This should materialize the offline-cache dir.
    target = "root//http_archive:test_zip"
    await bsmr.debug("trace-io", "enable")
    result = await bsmr.build(target)
    print("stderr:", result.stderr)
    assert "/offline-cache/" in result.stderr, (
        "materializer should declare offline-cache materialization"
    )

    # Validate that offline-cache path doesn't exist prior to manifest export.
    http_download_path = result.get_build_report().output_for_target(target)
    # This is hacky, but there's no other good way to discover the offline-cache path.
    offline_cache_path = (
        Path(str(http_download_path).replace("/art/", "/offline-cache/")).parent
        / "download"
    )
    assert not offline_cache_path.exists(), (
        "offline cache path should not exist before manifest export"
    )

    # Ensure bsmr-out/offline-cache paths are materialized.
    await bsmr.debug("trace-io", "export-manifest")
    assert offline_cache_path.exists(), (
        "offline cache path should exist after manifest export"
    )

    await bsmr.kill()

    result = await bsmr.build(
        "root//http_archive:test_zip",
        "--config",
        "bsmr.use_network_action_output_cache=true",
        "--no-remote-cache",
        "--local-only",
    )
    assert "LocalCopy" in result.stderr, "offline-cache path should be copied to output"
    assert http_download_path.exists(), "http download output path should exist"


@bsmr_test(skip_for_os=["windows"])
async def test_includes_cas_artifact_in_manifest(bsmr: Bsmr) -> None:
    hg_init(cwd=bsmr.cwd)

    _setup_bsmrconfig_digest_algorithms(bsmr)

    await bsmr.debug("trace-io", "enable")
    await bsmr.build("//cas_artifact:tree")
    out = await bsmr.debug("trace-io", "export-manifest")
    manifest = json.loads(out.stdout)

    assert any(
        re.match(
            r"bsmr-out\/.+\/offline-cache/root\/.+\/cas_artifact/__tree__/tree", path
        )
        is not None
        for path in manifest["paths"]
    ), "offline cache should contain cas artifact tree"

    assert_output_paths_materialized(bsmr.cwd, manifest["paths"])


# Ensure offline-cache bsmr-out dir is _not_ created when not doing I/O tracing.
@bsmr_test(skip_for_os=["windows"])
async def test_no_tracing_does_not_write_offline_cache_for_cas_artifact(
    bsmr: Bsmr,
) -> None:
    _setup_bsmrconfig_digest_algorithms(bsmr)

    await bsmr.build("//cas_artifact:tree")
    assert not os.path.exists(os.path.join(bsmr.cwd, "bsmr-out/offline-cache")), (
        "offline cache should not exist when not doing I/O tracing"
    )


# Validate that when bsmrconfig use_network_action_output_cache=true is set we use the
# offline-cache action output instead of fetching from the network.
@bsmr_test(
    skip_for_os=["windows"],
    extra_bsmr_config={"bsmr": {"sqlite_materializer_state": "false"}},
)
@env("BSMR_LOG", "bsmr_execute_impl::materializers=trace")
async def test_fake_offline_cas_artifact_uses_offline_cache(bsmr: Bsmr) -> None:
    hg_init(cwd=bsmr.cwd)

    _setup_bsmrconfig_digest_algorithms(bsmr)

    # This should materialize the offline-cache dir.
    target = "root//cas_artifact:tree"
    await bsmr.debug("trace-io", "enable")
    result = await bsmr.build(target)
    print("stderr:", result.stderr)
    assert "/offline-cache/" in result.stderr, (
        "materializer should declare offline-cache materialization"
    )

    # Validate that offline-cache path doesn't exist prior to manifest export.
    cas_download_path = result.get_build_report().output_for_target(target)
    # This is hacky, but there's no other good way to discover the offline-cache path.
    offline_cache_path = (
        Path(str(cas_download_path).replace("/art/", "/offline-cache/")).parent / "tree"
    )
    assert not offline_cache_path.exists(), (
        "offline cache path should not exist before manifest export"
    )

    # Ensure bsmr-out/offline-cache paths are materialized.
    await bsmr.debug("trace-io", "export-manifest")
    assert offline_cache_path.exists(), (
        "offline cache path should exist after manifest export"
    )

    await bsmr.kill()

    result = await bsmr.build(
        target,
        "--config",
        "bsmr.use_network_action_output_cache=true",
        "--no-remote-cache",
        "--local-only",
    )
    assert "LocalCopy" in result.stderr, "offline-cache path should be copied to output"
    assert cas_download_path.exists(), "cas action output path should exist"


# Validate that all lists in the exported manifest are sorted.
@bsmr_test(setup_eden=True, skip_for_os=["windows"])
@env("BSMR_HARD_ERROR", "false")
async def test_manifest_lists_are_sorted(bsmr: Bsmr) -> None:
    hg_config_reponame(cwd=bsmr.cwd)

    def symlink(link: str, target: str) -> None:
        """
        Symlinks link --> target. Assumes we're based in the bsmr cwd, so link must be relative.
        """
        os.symlink(target, os.path.join(bsmr.cwd, link))

    # Set up multiple files in reverse alphabetical order to ensure they need sorting
    symlink("symlinks/zz_last.cpp", "../hello_world/main.cpp")
    symlink("symlinks/aa_first.cpp", "../linking/main.cpp")
    symlink("symlinks/mm_middle.cpp", "../linking/static.cpp")

    with TemporaryDirectory() as tempdir:
        t = Path(tempdir)

        zz_file = t / "zz_external.h"
        zz_file.touch()

        aa_file = t / "aa_external.h"
        aa_file.touch()

        mm_file = t / "mm_external.h"
        mm_file.touch()

        symlink("symlinks/ext_1.h", str(zz_file))
        symlink("symlinks/ext_2.h", str(aa_file))
        symlink("symlinks/ext_3.h", str(mm_file))

        await bsmr.debug("trace-io", "enable")

        # Build multiple targets to create entries in non-alphabetical order
        await bsmr.build("root//symlinks:zz_last")
        await bsmr.build("root//symlinks:aa_first")
        await bsmr.build("root//symlinks:mm_middle")

        with NamedTemporaryFile("w", delete=False) as tmp1:
            tmpname1 = tmp1.name
            tmp1.write("[bsmr]\n")
            tmp1.write("  foo = bar\n")

        with NamedTemporaryFile("w", delete=False) as tmp2:
            tmpname2 = tmp2.name
            tmp2.write("[bsmr]\n")
            tmp2.write("  baz = qux\n")

        try:
            # Build with config files in reverse order to create unsorted external entries
            await bsmr.build("root//hello_world:welcome", "--config-file", tmpname2)
            await bsmr.build("root//hello_world:welcome", "--config-file", tmpname1)

            out = await bsmr.debug("trace-io", "export-manifest")
        finally:
            os.unlink(tmpname1)
            os.unlink(tmpname2)

    manifest = json.loads(out.stdout)

    paths = manifest["paths"]
    assert paths == sorted(paths), f"paths list is not sorted: {paths}"

    external_paths = manifest["external_paths"]
    assert external_paths == sorted(external_paths), (
        f"external_paths list is not sorted: {external_paths}"
    )

    relative_symlinks = manifest["relative_symlinks"]
    sorted_relative = sorted(relative_symlinks, key=lambda x: x["link"])
    assert relative_symlinks == sorted_relative, (
        f"relative_symlinks list is not sorted by link: {relative_symlinks}"
    )

    external_symlinks = manifest["external_symlinks"]
    sorted_external = sorted(external_symlinks, key=lambda x: x["link"])
    assert external_symlinks == sorted_external, (
        f"external_symlinks list is not sorted by link: {external_symlinks}"
    )

    assert_link_in(
        {"link": "symlinks/zz_last.cpp", "target": "hello_world/main.cpp"},
        relative_symlinks,
    )
    assert_link_in(
        {"link": "symlinks/aa_first.cpp", "target": "linking/main.cpp"},
        relative_symlinks,
    )
    assert_link_in(
        {"link": "symlinks/mm_middle.cpp", "target": "linking/static.cpp"},
        relative_symlinks,
    )


@bsmr_test(
    skip_for_os=["windows"],
    extra_bsmr_config={"bsmr": {"sqlite_materializer_state": "false"}},
)
@env("BSMR_LOG", "bsmr_execute_impl::materializers=trace")
async def test_run_action_with_allow_offline_output_cache(bsmr: Bsmr) -> None:
    """Test RunAction caching when allow_offline_output_cache=True."""
    hg_init(cwd=bsmr.cwd)

    target = "root//run_action_cache:cached_target"

    # Build with trace mode to populate offline cache
    await bsmr.debug("trace-io", "enable")
    result = await bsmr.build(target)
    print("stderr:", result.stderr)
    assert "/offline-cache/" in result.stderr, (
        "materializer should declare offline-cache materialization"
    )

    # Get output path
    output_path = result.get_build_report().output_for_target(target)

    # Compute offline cache path (hacky but same as other tests)
    offline_cache_path = (
        Path(str(output_path).replace("/art/", "/offline-cache/")).parent / "out.txt"
    )
    assert not offline_cache_path.exists(), (
        "offline cache path should not exist before manifest export"
    )

    # Export manifest to materialize offline-cache
    await bsmr.debug("trace-io", "export-manifest")
    assert offline_cache_path.exists(), (
        "offline cache path should exist after manifest export"
    )

    await bsmr.kill()

    # Rebuild with offline cache enabled
    result = await bsmr.build(
        target,
        "--config",
        "bsmr.use_network_action_output_cache=true",
        "--no-remote-cache",
        "--local-only",
    )
    assert "LocalCopy" in result.stderr, "offline-cache path should be copied to output"
    assert output_path.exists(), "action output path should exist"


@bsmr_test(skip_for_os=["windows"])
async def test_run_action_without_parameter_does_not_cache(bsmr: Bsmr) -> None:
    """Test that RunAction without allow_offline_output_cache doesn't cache."""
    hg_init(cwd=bsmr.cwd)

    await bsmr.debug("trace-io", "enable")
    await bsmr.build("root//run_action_cache:uncached_target")
    out = await bsmr.debug("trace-io", "export-manifest")
    manifest = json.loads(out.stdout)

    # Verify that offline-cache paths do NOT include uncached_target
    offline_cache_paths = [
        path
        for path in manifest["paths"]
        if "/offline-cache/" in path and "uncached_target" in path
    ]
    assert len(offline_cache_paths) == 0, (
        f"uncached target should not appear in offline-cache: {offline_cache_paths}"
    )


@bsmr_test(skip_for_os=["windows"])
async def test_run_action_cache_includes_in_manifest(bsmr: Bsmr) -> None:
    """Test that cached RunAction outputs appear in trace manifest."""
    hg_init(cwd=bsmr.cwd)

    await bsmr.debug("trace-io", "enable")
    await bsmr.build("root//run_action_cache:cached_target")
    out = await bsmr.debug("trace-io", "export-manifest")
    manifest = json.loads(out.stdout)

    assert any(
        re.match(
            r"bsmr-out/.+/offline-cache/root/.+/run_action_cache/__cached_target__/out.txt",
            path,
        )
        is not None
        for path in manifest["paths"]
    ), "offline cache should contain cached run action output"

    assert_output_paths_materialized(bsmr.cwd, manifest["paths"])


@bsmr_test(
    skip_for_os=["windows"],
    extra_bsmr_config={"bsmr": {"sqlite_materializer_state": "false"}},
)
@env("BSMR_LOG", "bsmr_execute_impl::materializers=trace")
async def test_genrule_with_allow_offline_output_cache(bsmr: Bsmr) -> None:
    """Test genrule caching when allow_offline_output_cache=True."""
    hg_init(cwd=bsmr.cwd)

    target = "root//genrule_cache:cached"

    # Build with trace mode to populate offline cache
    await bsmr.debug("trace-io", "enable")
    result = await bsmr.build(target)
    print("stderr:", result.stderr)
    assert "/offline-cache/" in result.stderr, (
        "materializer should declare offline-cache materialization"
    )

    # Get output path
    output_path = result.get_build_report().output_for_target(target)

    # Compute offline cache path
    offline_cache_path = (
        Path(str(output_path).replace("/art/", "/offline-cache/")).parent / "output.txt"
    )
    assert not offline_cache_path.exists(), (
        "offline cache path should not exist before manifest export"
    )

    # Export manifest to materialize offline-cache
    await bsmr.debug("trace-io", "export-manifest")
    assert offline_cache_path.exists(), (
        "offline cache path should exist after manifest export"
    )

    await bsmr.kill()

    # Rebuild with offline cache enabled
    result = await bsmr.build(
        target,
        "--config",
        "bsmr.use_network_action_output_cache=true",
        "--no-remote-cache",
        "--local-only",
    )
    assert "LocalCopy" in result.stderr, "offline-cache path should be copied to output"
    assert output_path.exists(), "genrule output path should exist"


@bsmr_test(skip_for_os=["windows"])
async def test_genrule_without_parameter_does_not_cache(bsmr: Bsmr) -> None:
    """Test that genrule without allow_offline_output_cache doesn't cache."""
    hg_init(cwd=bsmr.cwd)

    await bsmr.debug("trace-io", "enable")
    await bsmr.build("root//genrule_cache:uncached")
    out = await bsmr.debug("trace-io", "export-manifest")
    manifest = json.loads(out.stdout)

    # Verify that offline-cache paths do NOT include uncached genrule
    offline_cache_paths = [
        path
        for path in manifest["paths"]
        if "/offline-cache/" in path and "uncached" in path
    ]
    assert len(offline_cache_paths) == 0, (
        f"uncached genrule should not appear in offline-cache: {offline_cache_paths}"
    )


@bsmr_test(skip_for_os=["windows"])
async def test_genrule_cache_includes_in_manifest(bsmr: Bsmr) -> None:
    """Test that cached genrule outputs appear in trace manifest."""
    hg_init(cwd=bsmr.cwd)

    await bsmr.debug("trace-io", "enable")
    await bsmr.build("root//genrule_cache:cached")
    out = await bsmr.debug("trace-io", "export-manifest")
    manifest = json.loads(out.stdout)

    assert any(
        re.match(
            r"bsmr-out/.+/offline-cache/root/.+/genrule_cache/__cached__/output.txt",
            path,
        )
        is not None
        for path in manifest["paths"]
    ), "offline cache should contain cached genrule output"

    assert_output_paths_materialized(bsmr.cwd, manifest["paths"])


# No-op test for windows.
@bsmr_test()
async def test_noop(bsmr: Bsmr) -> None:
    return

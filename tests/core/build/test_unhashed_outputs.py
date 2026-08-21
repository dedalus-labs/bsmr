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

import os
import shutil
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_unhashed_putputs(bsmr: Bsmr) -> None:
    await bsmr.build("//pack:trivial_build")

    p = bsmr.cwd / "bsmr-out" / "v2" / "gen" / "root" / "pack" / "foo.txt"
    assert p.exists()
    assert p.is_symlink()


@bsmr_test()
async def test_projected_output(bsmr: Bsmr) -> None:
    await bsmr.build("//:projected_output")

    p = bsmr.cwd / "bsmr-out" / "v2" / "gen" / "root" / "dir"
    assert p.exists()
    assert p.is_symlink()
    assert (p / "file").is_file()


@bsmr_test()
async def test_build_symlink_does_not_traverse_existing_symlinks(bsmr: Bsmr) -> None:
    await bsmr.build("//pack:trivial_build")
    symlink_folder = bsmr.cwd / "bsmr-out" / "v2" / "gen" / "root" / "pack"

    # Now, overwrite part of the symlink path with something we cannot traverse.
    path = symlink_folder.parent
    shutil.rmtree(path)
    # On Windows this is just non existing path.
    os.symlink("/dev/null", path)

    # Can we still build? If we delete the symlink when walking up the path, we
    # can. If we traverse it, we can't.
    await bsmr.build("//pack:trivial_build")


@bsmr_test()
async def test_conflict_with_content_based_paths(bsmr: Bsmr) -> None:
    symlink_path: Path = (
        bsmr.cwd / "bsmr-out" / "v2" / "gen" / "root" / "conflict" / "shared_name"
    )
    content_based_path: Path = (
        bsmr.cwd / "bsmr-out" / "v2" / "art" / "root" / "conflict" / "shared_name"
    )
    subtarget_output: Path
    # sanity check that we're starting from a clean state
    assert not symlink_path.exists()
    assert not content_based_path.exists()

    def base_checks(*, should_symlink_exist: bool) -> None:
        if should_symlink_exist:
            assert symlink_path.is_symlink()
            assert symlink_path.readlink().is_file()
        else:
            assert not symlink_path.exists()

        assert content_based_path.is_dir()
        assert subtarget_output.is_symlink()
        assert subtarget_output.resolve().is_file()
        assert not subtarget_output.resolve().is_relative_to(symlink_path)
        assert subtarget_output.resolve().is_relative_to(content_based_path)
        # Verify we can read the contents of the file
        with open(subtarget_output) as f:
            f.read()

    #
    # Build just the subtarget. Esnsure that the subtarget output exists and is
    # reacable, and that it lives in the place we expect.
    #
    res = await bsmr.build(
        "//conflict/shared_name:subtarget",
        "--config",
        "bsmr.create_unhashed_links=false",
    )
    subtarget_output = res.get_build_report().output_for_target(
        "root//conflict/shared_name:subtarget"
    )
    base_checks(should_symlink_exist=False)

    #
    # Build the conflicting target w/o unhashed links. This should leave the
    # subtarget_output alone, which should remain readable.
    #
    await bsmr.build(
        "//conflict:shared_name",
        "--config",
        "bsmr.create_unhashed_links=false",
    )
    # TODO(jtbraun): this should instead ensure the symlink does NOT exist, and the content_based_path does and is a folder
    base_checks(should_symlink_exist=False)

    #
    # Build the conflicting target with unhashed links. This will overwrite the
    # subtarget with a directory, and the symlink_path will now exist.
    #
    await bsmr.build("//conflict:shared_name")
    base_checks(should_symlink_exist=True)

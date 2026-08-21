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

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_audit_file_package_simple(bsmr: Bsmr) -> None:
    """Test basic file-package mapping"""
    result = await bsmr.audit("file-package", "TARGETS.fixture")
    assert ": root//" in result.stdout


@bsmr_test()
async def test_audit_file_package_json(bsmr: Bsmr) -> None:
    """Test file-package mapping with JSON output"""
    result = await bsmr.audit("file-package", "TARGETS.fixture", "--json")

    data = json.loads(result.stdout)
    expected = {"TARGETS.fixture": {"package": "root//"}}
    assert data == expected, f"Expected {expected}, got {data}"


@bsmr_test()
async def test_audit_file_package_newcell_json(bsmr: Bsmr) -> None:
    """Test file-package mapping for a file in 'newcell'"""
    # Assume 'newcell/TARGETS.fixture' exists in the test workspace
    result = await bsmr.audit("file-package", "newcell/TARGETS.fixture", "--json")

    data = json.loads(result.stdout)
    expected = {"newcell/TARGETS.fixture": {"package": "newcell//"}}
    assert data == expected, f"Expected {expected}, got {data}"


@bsmr_test()
async def test_audit_file_package_multiple_paths_json(bsmr: Bsmr) -> None:
    """Test file-package mapping with multiple paths, including a file in 'newcell'"""
    result = await bsmr.audit(
        "file-package",
        "TARGETS.fixture",
        "subdir/testfile",
        "newcell/TARGETS.fixture",
        "--json",
    )

    data = json.loads(result.stdout)
    expected = {
        "TARGETS.fixture": {"package": "root//"},
        "subdir/testfile": {"package": "root//subdir"},
        "newcell/TARGETS.fixture": {"package": "newcell//"},
    }
    assert data == expected, f"Expected {expected}, got {data}"


@bsmr_test()
async def test_audit_file_package_with_errors_json(bsmr: Bsmr) -> None:
    """Test file-package mapping with a mix of valid and invalid paths"""
    result = await bsmr.audit(
        "file-package",
        "TARGETS.fixture",
        "nonexistent/file.txt",
        "newcell/TARGETS.fixture",
        "--json",
    )

    data = json.loads(result.stdout)
    expected = {
        "TARGETS.fixture": {"package": "root//"},
        "newcell/TARGETS.fixture": {"package": "newcell//"},
        "nonexistent/file.txt": {"error": "Error listing dir `nonexistent`"},
    }
    assert data == expected, f"Expected {expected}, got {data}"


@bsmr_test()
async def test_audit_file_package_with_errors_plain(bsmr: Bsmr) -> None:
    """Test file-package mapping with a mix of valid and invalid paths (plain text)"""
    result = await bsmr.audit(
        "file-package",
        "TARGETS.fixture",
        "nonexistent/file.txt",
        "newcell/TARGETS.fixture",
    )

    # Verify successful paths are in the output with correct format
    assert "TARGETS.fixture: root//" in result.stdout
    assert "newcell/TARGETS.fixture: newcell//" in result.stdout

    # Verify error path shows error message
    assert "nonexistent/file.txt: Error:" in result.stdout


@bsmr_test()
async def test_audit_file_package_absolute_path(bsmr: Bsmr) -> None:
    """Test file-package mapping with an absolute path"""
    abs_path = str(bsmr.cwd / "TARGETS.fixture")
    result = await bsmr.audit("file-package", abs_path, "--json")

    data = json.loads(result.stdout)
    expected = {abs_path: {"package": "root//"}}
    assert data == expected, f"Expected {expected}, got {data}"

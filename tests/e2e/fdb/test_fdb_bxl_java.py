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


from bsmr.tests.e2e.fdb.types import ExecInfo
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_java_test(bsmr: Bsmr) -> None:
    root = (await bsmr.root("--kind", "project")).stdout.strip("\n")
    result = await bsmr.bxl(
        "prelude//debugging/fdb.bxl:inspect_target",
        "--",
        "--target",
        "root//tests/targets/rules/java/java_test:simple_junit_test_java11",
    )

    exec_info = ExecInfo.from_bsmr_result(result)
    classmap = exec_info.read_class_map(root)
    names = [class_ref.name for entry in classmap for class_ref in entry.classes]
    assert names == ["com.example.SimpleJUnitTest"]


@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_java_binary(bsmr: Bsmr) -> None:
    root = (await bsmr.root("--kind", "project")).stdout.strip("\n")
    result = await bsmr.bxl(
        "prelude//debugging/fdb.bxl:inspect_target",
        "--",
        "--target",
        "root//tests/targets/rules/java/good/java_binary_with_native_libs:binary_with_native_lib",
    )
    exec_info: ExecInfo = ExecInfo.from_bsmr_result(result)
    classmap = exec_info.read_class_map(root)
    names = [class_ref.name for entry in classmap for class_ref in entry.classes]
    assert names == ["JavaBinaryWithNativeLibs"]


@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_java_library(bsmr: Bsmr) -> None:
    root = (await bsmr.root("--kind", "project")).stdout.strip("\n")
    result = await bsmr.bxl(
        "prelude//debugging/fdb.bxl:inspect_target",
        "--",
        "--target",
        "root//tests/targets/rules/java/good/java_binary_with_native_libs:lib",
    )
    exec_info: ExecInfo = ExecInfo.from_bsmr_result(result)
    classmap = exec_info.read_class_map(root)
    names = [class_ref.name for entry in classmap for class_ref in entry.classes]
    assert names == ["JavaBinaryWithNativeLibs"]


@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_kotlin_test(bsmr: Bsmr) -> None:
    root = (await bsmr.root("--kind", "project")).stdout.strip("\n")
    result = await bsmr.bxl(
        "prelude//debugging/fdb.bxl:inspect_target",
        "--",
        "--target",
        "root//tests/targets/rules/kotlin/kotlin_test:simple_kotlin_test",
    )
    exec_info: ExecInfo = ExecInfo.from_bsmr_result(result)
    classmap = exec_info.read_class_map(root)
    names = [class_ref.name for entry in classmap for class_ref in entry.classes]
    assert names == ["com.example.SimpleKotlinTest"]


@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_kotlin_library(bsmr: Bsmr) -> None:
    root = (await bsmr.root("--kind", "project")).stdout.strip("\n")
    result = await bsmr.bxl(
        "prelude//debugging/fdb.bxl:inspect_target",
        "--",
        "--target",
        "root//tests/targets/rules/kotlin/kotlin_library:lib_with_source_only_abi_generation",
    )
    exec_info: ExecInfo = ExecInfo.from_bsmr_result(result)
    classmap = exec_info.read_class_map(root)
    names = [class_ref.name for entry in classmap for class_ref in entry.classes]
    assert names == ["A", "B"]


@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_apk_gen_rule(bsmr: Bsmr) -> None:
    root = (await bsmr.root("--kind", "project")).stdout.strip("\n")
    result = await bsmr.bxl(
        "prelude//debugging/fdb.bxl:inspect_target",
        "--",
        "--target",
        "upstream//fbandroid/bsmr/tests/good/apk:zip_align_basic_apk",
    )
    exec_info: ExecInfo = ExecInfo.from_bsmr_result(result)
    classmap = exec_info.read_class_map(root)
    names = [class_ref.name for entry in classmap for class_ref in entry.classes]
    assert names == [
        "com.example.sampleapp.MainActivity",
        "com.example.sampleapp.Helper",
        "com.example.sampleapp.Helper$SomeInterface",
    ]


@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_instrumentation_test(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "prelude//debugging/fdb.bxl:inspect_target",
        "--",
        "--target",
        "upstream//fbandroid/bsmr/tests/good/instrumentation_test:single_apk_test",
    )
    exec_info: ExecInfo = ExecInfo.from_bsmr_result(result)
    assert any("args_file" in str(arg) for arg in exec_info.data["program"])


# This is to ensure at least one of the tests is passing on Windows otherwise CI fails
@bsmr_test(inplace=True)
async def test_noop(bsmr: Bsmr) -> None:
    return

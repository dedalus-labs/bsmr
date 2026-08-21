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
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test(inplace=False)
async def test_bare_select_typecheck(bsmr: Bsmr) -> None:
    """Bare Select type annotations should pass type checking."""
    await bsmr.starlark("typecheck", "select_types.bzl")


@bsmr_test(inplace=False)
async def test_select_union_accepts_both(bsmr: Bsmr) -> None:
    """Select[str] | str accepts both select expressions and plain strings.

    TODO(D94266514): Currently vacuous because Select[T] resolves to Any.
    """
    await bsmr.starlark("typecheck", "select_union_good.bzl")


@bsmr_test(inplace=False)
async def test_select_rejects_plain_value(bsmr: Bsmr) -> None:
    """Select[str] rejects a plain str argument during type checking.

    TODO(D94266514): Once Select[T] type parameterization lands, this should
    use expect_failure with stderr_regex="Detected 2 errors".
    Currently Select[T] resolves to Any so no type error is produced.
    """
    # TODO(D94266514): expect_failure once Select[T] lands
    await bsmr.starlark("typecheck", "select_union_bad.bzl")


@bsmr_test(inplace=False)
async def test_nested_type_params(bsmr: Bsmr) -> None:
    """Nested type parameters like Select[list[str]] pass type checking.

    TODO(D94266514): Currently vacuous because Select[T] resolves to Any.
    """
    await bsmr.starlark("typecheck", "select_nested_good.bzl")


@bsmr_test(inplace=False)
async def test_nested_type_params_reject_plain(bsmr: Bsmr) -> None:
    """Select[list[str]] rejects plain values during type checking.

    TODO(D94266514): Once Select[T] type parameterization lands, this should
    use expect_failure with stderr_regex="Detected 2 errors".
    """
    # TODO(D94266514): expect_failure once Select[T] lands
    await bsmr.starlark("typecheck", "select_nested_bad.bzl")


@bsmr_test(inplace=False)
async def test_optional_type_params(bsmr: Bsmr) -> None:
    """Select[str | None] accepts select expressions resolving to str or None.

    TODO(D94266514): Currently vacuous because Select[T] resolves to Any.
    """
    await bsmr.starlark("typecheck", "select_optional_good.bzl")


@bsmr_test(inplace=False)
async def test_optional_type_params_reject_plain(bsmr: Bsmr) -> None:
    """Select[str | None] rejects plain values during type checking.

    TODO(D94266514): Once Select[T] type parameterization lands, this should
    use expect_failure with stderr_regex="Detected 2 errors".
    """
    # TODO(D94266514): expect_failure once Select[T] lands
    await bsmr.starlark("typecheck", "select_optional_bad.bzl")


@bsmr_test(inplace=False)
async def test_select_bool_parameterization(bsmr: Bsmr) -> None:
    """Select[bool] accepts select expressions with boolean branches.

    TODO(D94266514): Currently vacuous because Select[T] resolves to Any.
    """
    await bsmr.starlark("typecheck", "select_bool_good.bzl")


@bsmr_test(inplace=False)
async def test_select_bool_rejects_plain(bsmr: Bsmr) -> None:
    """Select[bool] rejects plain boolean and string values.

    TODO(D94266514): Once Select[T] type parameterization lands, this should
    use expect_failure with stderr_regex="Detected 2 errors".
    """
    # TODO(D94266514): expect_failure once Select[T] lands
    await bsmr.starlark("typecheck", "select_bool_bad.bzl")


@bsmr_test(inplace=False)
async def test_select_str_not_equal_to_union(bsmr: Bsmr) -> None:
    """Select[str] alone accepts select expressions but not plain values.

    TODO(D94266514): Currently vacuous because Select[T] resolves to Any.
    """
    await bsmr.starlark("typecheck", "select_semantics_good.bzl")


@bsmr_test(inplace=False)
async def test_select_str_rejects_plain_proving_not_union(bsmr: Bsmr) -> None:
    """Select[str] is not Select[str] | str: plain values are rejected.

    TODO(D94266514): Once Select[T] type parameterization lands, this should
    use expect_failure with stderr_regex="Detected 2 errors".
    """
    # TODO(D94266514): expect_failure once Select[T] lands
    await bsmr.starlark("typecheck", "select_semantics_bad.bzl")


@bsmr_test(inplace=False)
async def test_branch_type_matching(bsmr: Bsmr) -> None:
    """select() branch values matching the type parameter pass type checking.

    TODO(D94266514): Currently vacuous because Select[T] resolves to Any.
    """
    await bsmr.starlark("typecheck", "select_branch_good.bzl")


@bsmr_test(inplace=False)
async def test_branch_type_mismatch(bsmr: Bsmr) -> None:
    """select() branch values not matching the type parameter are rejected.

    TODO(D94266514): Once Select[T] type parameterization lands, this should
    use expect_failure with stderr_regex="Detected 2 errors".
    """
    # TODO(D94266514): expect_failure once Select[T] lands
    await bsmr.starlark("typecheck", "select_branch_bad.bzl")


@bsmr_test(inplace=False)
async def test_backward_compat_bare_select(bsmr: Bsmr) -> None:
    """Bare Select type accepts any select() regardless of branch value types."""
    await bsmr.starlark("typecheck", "select_compat_good.bzl")


@bsmr_test(inplace=False)
async def test_rbin_op_list_plus_select(bsmr: Bsmr) -> None:
    """list + Select[T] type-checks via rbin_op dispatch on custom types.

    TODO(D94266514): Currently vacuous because Select[T] resolves to Any.
    """
    await bsmr.starlark("typecheck", "select_rbin_op_good.bzl")


@bsmr_test(inplace=False)
async def test_select_or_list_union_accepts_both(bsmr: Bsmr) -> None:
    """Select[str] | list[str] accepts both select expressions and plain lists.

    TODO(D94266514): Currently vacuous because Select[T] resolves to Any.
    """
    await bsmr.starlark("typecheck", "select_union_param_good.bzl")


@bsmr_test(inplace=False)
async def test_select_or_list_union_rejects_wrong_type(bsmr: Bsmr) -> None:
    """Select[str] | list[str] rejects plain int and str values.

    TODO(D94266514): Once Select[T] type parameterization lands, this should
    use expect_failure with stderr_regex="Detected 2 errors".
    """
    # TODO(D94266514): expect_failure once Select[T] lands
    await bsmr.starlark("typecheck", "select_union_param_bad.bzl")


@bsmr_test(inplace=False)
async def test_select_intersect_accepts_matching(bsmr: Bsmr) -> None:
    """Select[str] | Select[int] accepts select expressions with str or int branches."""
    await bsmr.starlark("typecheck", "select_intersect_good.bzl")


@bsmr_test(inplace=False)
async def test_select_intersect_rejects_non_overlapping(bsmr: Bsmr) -> None:
    """Select[str] | Select[int] rejects Select[bool] and plain values.

    TODO(D94266514): Once Select[T] type parameterization lands, this should
    use expect_failure with stderr_regex="Detected 2 errors".
    """
    # TODO(D94266514): expect_failure once Select[T] lands
    await bsmr.starlark("typecheck", "select_intersect_bad.bzl")

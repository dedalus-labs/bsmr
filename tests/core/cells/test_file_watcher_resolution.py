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


@bsmr_test()
async def test_changing_cell_location_bug(bsmr: Bsmr) -> None:
    await bsmr.targets("foo//:", "bar//:")

    # Switch the location of the 2 cells
    (bsmr.cwd / ".bsmr").write_text(
        "[cells]\nfoo=bar\nbar=foo\nroot=.\nprelude=.\n"
    )

    # Make sure bsmr picks up the `CellResolver` updates
    await bsmr.targets("foo//:", "bar//:")

    (bsmr.cwd / "foo" / "TARGETS.fixture").write_text("fail('error')")

    # FIXME(JakobDegen): The change to the `TARGETS.fixture` file does not get picked up by bsmr.
    # The cause is that the file watcher always invalidates injected keys computed from `CellPath`s,
    # but the `CellResolver` that it uses to map `ProjectRelativePath`s to `CellPath`s is computed
    # once at daemon startup and never updated. So concretely, the file update above results in the
    # cell path `bar//TARGETS.fixture` being invalidated, which means the targets in `foo//:` are
    # never recomputed.
    #
    # This is just one example, there's a thousand other ways that you can change the `CellResolver`
    # to create similar bugs.
    await bsmr.targets("foo//:", "bar//:")

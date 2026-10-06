# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Exposes exact executable bytes for OCI integration fixtures.


def _artifact(ctx: AnalysisContext) -> list[Provider]:
    """Track one executable artifact without an ambient command lookup."""
    providers = [DefaultInfo(default_output=ctx.attrs.binary), RunInfo(args=[ctx.attrs.binary])]
    return providers


artifact = rule(impl=_artifact, attrs={"binary": attrs.source()})

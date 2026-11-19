# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Exposes exact executable bytes for OCI integration fixtures.

def _artifact(ctx):
    """Track one executable artifact without an ambient command lookup."""
    return [DefaultInfo(default_output = ctx.attrs.binary), RunInfo(args = [ctx.attrs.binary])]

artifact = rule(impl = _artifact, attrs = {"binary": attrs.source()})

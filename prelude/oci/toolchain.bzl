# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Selects artifact-backed OCI tools on the action's execution platform.

load(":providers.bzl", "OciToolchainInfo")
load("@prelude//:artifacts.bzl", "single_artifact")

def _oci_toolchain_impl(ctx: AnalysisContext) -> list[Provider]:
    """Bind qualified encoder/runtime artifacts and the complete helper module closure."""
    if ctx.attrs.version != "v0.3.22":
        fail("oci_toolchain requires the qualified rules_img version v0.3.22")
    return [
        DefaultInfo(),
        OciToolchainInfo(
            img = single_artifact(ctx.attrs.img).default_output,
            operations = RunInfo(args = cmd_args(
                single_artifact(ctx.attrs.node).default_output, ctx.attrs._main, hidden = [ctx.attrs._closure],
            )),
        ),
    ]

oci_toolchain = rule(
    impl = _oci_toolchain_impl,
    attrs = {
        "img": attrs.exec_dep(),
        "node": attrs.exec_dep(),
        "version": attrs.string(default = "v0.3.22"),
        "_main": attrs.default_only(attrs.source(default = "prelude//oci:operations")),
        "_closure": attrs.default_only(attrs.source(default = "prelude//oci:closure")),
    },
    is_toolchain_rule = True,
    doc = "Binds a pinned img encoder and Node runtime to the bundled OCI operations.",
)

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
            node = single_artifact(ctx.attrs.node).default_output,
            umoci = single_artifact(ctx.attrs.umoci).default_output if ctx.attrs.umoci != None else None,
            runc = single_artifact(ctx.attrs.runc).default_output if ctx.attrs.runc != None else None,
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
        "umoci": attrs.option(attrs.exec_dep(), default = None, doc = "Pinned Linux image unpack/repack tool; required by oci_run."),
        "runc": attrs.option(attrs.exec_dep(), default = None, doc = "Pinned Linux OCI runtime; required by oci_run."),
        "version": attrs.string(default = "v0.3.22"),
        "_main": attrs.default_only(attrs.source(default = "prelude//oci:operations")),
        "_closure": attrs.default_only(attrs.source(default = "prelude//oci:closure")),
    },
    is_toolchain_rule = True,
    doc = "Binds a pinned img encoder and Node runtime to the bundled OCI operations.",
)

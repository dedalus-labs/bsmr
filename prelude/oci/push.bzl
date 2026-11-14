# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Separates cacheable publication preparation from the explicit registry mutation.

load(":providers.bzl", "OciImageInfo", "OciToolchainInfo", "oci_layout_spec")

def _oci_push_impl(ctx: AnalysisContext) -> list[Provider]:
    """Prepare exact image bytes; publish them only when the target is run."""
    image = ctx.attrs.image[OciImageInfo]
    toolchain = ctx.attrs.toolchain[OciToolchainInfo]
    parts = ctx.attrs.repository.split("/", 1)
    if len(parts) != 2 or not parts[0] or not parts[1] or ":" in parts[1] or "://" in ctx.attrs.repository or "@" in ctx.attrs.repository:
        fail("OCI_PUSH_INVALID_REPOSITORY: use registry/repository without a tag or digest")
    if len({tag: True for tag in ctx.attrs.tags}) != len(ctx.attrs.tags):
        fail("OCI_PUSH_INVALID_TAGS: publication tags must be unique")
    layout = ctx.actions.declare_output("layout", dir = True)
    spec = ctx.actions.write_json("layout-inputs.json", oci_layout_spec(image), with_inputs = True)
    ctx.actions.run(cmd_args(
        toolchain.operations, "layout", "--img", toolchain.img, "--platform", image.platform,
        "--spec", spec, "--output", layout.as_output(),
    ), category = "oci_push_layout", allow_cache_upload = False, allow_local_cache_upload = False)
    config = ctx.actions.write_json("destination.json", {
        "registry": parts[0], "repository": parts[1], "tags": ctx.attrs.tags,
    })
    request = ctx.actions.declare_output("push.json")
    ctx.actions.run(cmd_args(
        toolchain.img, "deploy-metadata", "--command", "push", "--root-kind", "manifest",
        "--root-path", image.manifest, "--manifest-path", cmd_args(image.manifest, format = "0={}"),
        "--configuration-file", config, "--strategy", "eager", request.as_output(),
    ), category = "oci_push_metadata", allow_cache_upload = True)
    return [
        DefaultInfo(default_output = request),
        RunInfo(args = cmd_args(toolchain.node, ctx.attrs._push, "--img", toolchain.img,
            "--request", request, "--layout", layout, hidden = [ctx.attrs._auth, ctx.attrs._client])),
    ]

oci_push = rule(impl = _oci_push_impl, attrs = {
    "image": attrs.dep(providers = [OciImageInfo]),
    "repository": attrs.string(),
    "tags": attrs.list(attrs.string(), default = []),
    "toolchain": attrs.toolchain_dep(providers = [OciToolchainInfo]),
    "_push": attrs.default_only(attrs.source(default = "prelude//oci:push")),
    "_auth": attrs.default_only(attrs.source(default = "prelude//oci:auth")),
    "_client": attrs.default_only(attrs.source(default = "prelude//oci:client")),
}, doc = "Prepare an image with build; explicitly publish its configured tags with bsmr run. Publication is never a cached action.")

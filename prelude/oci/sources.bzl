# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Acquires locked registry images without placing credentials in build actions.

load(":providers.bzl", "OciImageInfo", "OciToolchainInfo", "oci_platform")

def _oci_pull_impl(ctx: AnalysisContext) -> list[Provider]:
    """Acquire only when requested, tracking both the immutable lock and helper closure."""
    platform = oci_platform(ctx.attrs.platform)
    toolchain = ctx.attrs.toolchain[OciToolchainInfo]
    layout = ctx.actions.declare_output("layout", dir = True)
    manifest = ctx.actions.declare_output("manifest.json")
    config = ctx.actions.declare_output("config.json")
    descriptor = ctx.actions.declare_output("descriptor.json")
    spec = ctx.actions.write_json("pull.json", {
        "image": ctx.attrs.image,
        "platform": platform,
        "lock": ctx.attrs.lock,
    }, with_inputs = True)
    ctx.actions.run(cmd_args(
        toolchain.node, ctx.attrs._source, "--img", toolchain.img, "--spec", spec,
        "--output", layout.as_output(), "--manifest", manifest.as_output(),
        "--config", config.as_output(), "--descriptor", descriptor.as_output(),
        hidden = [ctx.attrs._closure, ctx.attrs._auth],
    ), category = "oci_pull", local_only = True, allow_cache_upload = True, allow_local_cache_upload = True)
    return [
        DefaultInfo(default_output = descriptor, sub_targets = {
            "layout": [DefaultInfo(default_output = layout)],
            "manifest": [DefaultInfo(default_output = manifest)],
            "config": [DefaultInfo(default_output = config)],
        }),
        OciImageInfo(manifest = manifest, config = config, descriptor = descriptor,
            platform = platform, layers = [], layouts = [layout]),
    ]

_source_attrs = {
    "image": attrs.string(),
    "platform": attrs.string(),
    "lock": attrs.source(),
    "toolchain": attrs.toolchain_dep(providers = [OciToolchainInfo]),
    "_source": attrs.default_only(attrs.source(default = "prelude//oci:sources")),
    "_closure": attrs.default_only(attrs.source(default = "prelude//oci:closure")),
    "_auth": attrs.default_only(attrs.source(default = "prelude//oci:auth")),
}

oci_pull = rule(impl = _oci_pull_impl, attrs = _source_attrs,
    doc = "Acquires a digest-locked public image anonymously as a verified OCI provider.")

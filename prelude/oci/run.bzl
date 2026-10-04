# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Runs declared Linux commands against private OCI root filesystems.

load(":providers.bzl", "OciImageInfo", "OciToolchainInfo", "oci_layout_spec")

def _oci_run_impl(ctx: AnalysisContext) -> list[Provider]:
    """Export a base, run its filesystem command, and expose validated result metadata."""
    image = ctx.attrs.base[OciImageInfo]
    toolchain = ctx.attrs.toolchain[OciToolchainInfo]
    if toolchain.umoci == None or toolchain.runc == None:
        fail("MissingOciRuntime: oci_run requires declared umoci and runc executable artifacts")
    if not ctx.attrs.command:
        fail("OCI_RUN_INVALID_SPEC: oci_run requires a nonempty command argv")
    base = ctx.actions.declare_output("base-layout", dir = True)
    closure = ctx.actions.write_json("base-inputs.json", oci_layout_spec(image), with_inputs = True)
    ctx.actions.run(cmd_args(
        toolchain.operations, "layout", "--img", toolchain.img, "--platform", image.platform,
        "--spec", closure, "--output", base.as_output(),
    ), category = "oci_run_base", allow_cache_upload = True)
    spec = ctx.actions.write_json("run-inputs.json", {
        "layout": base,
        "platform": image.platform,
        "inputs": ctx.attrs.inputs,
        "command": ctx.attrs.command,
        "env": ctx.attrs.env,
        "user": ctx.attrs.user,
        "working_dir": ctx.attrs.working_dir,
    }, with_inputs = True)
    output = ctx.actions.declare_output("layout", dir = True)
    ctx.actions.run(cmd_args(
        toolchain.node, ctx.attrs._run, "--umoci", toolchain.umoci, "--runc", toolchain.runc,
        "--spec", spec, "--output", output.as_output(), hidden = [ctx.attrs._closure],
    ), category = "oci_run", local_only = True, allow_cache_upload = False, allow_local_cache_upload = True)
    manifest = ctx.actions.declare_output("manifest.json")
    config = ctx.actions.declare_output("config.json")
    descriptor = ctx.actions.declare_output("descriptor.json")
    ctx.actions.run(cmd_args(
        toolchain.operations, "import", "--platform", image.platform, "--layout", output,
        "--manifest", manifest.as_output(), "--config", config.as_output(), "--descriptor", descriptor.as_output(),
    ), category = "oci_run_import", allow_cache_upload = True)
    return [
        DefaultInfo(default_output = descriptor, sub_targets = {
            "layout": [DefaultInfo(default_output = output)],
            "manifest": [DefaultInfo(default_output = manifest)],
            "config": [DefaultInfo(default_output = config)],
        }),
        OciImageInfo(manifest = manifest, config = config, descriptor = descriptor,
            platform = image.platform, layers = [], layouts = [output]),
    ]

oci_run = rule(impl = _oci_run_impl, attrs = {
    "base": attrs.dep(providers = [OciImageInfo]),
    "inputs": attrs.dict(key = attrs.string(), value = attrs.source(allow_directory = True), default = {}),
    "command": attrs.list(attrs.string()),
    "env": attrs.dict(key = attrs.string(), value = attrs.string(), default = {}),
    "user": attrs.option(attrs.string(), default = None),
    "working_dir": attrs.option(attrs.string(), default = None),
    "toolchain": attrs.toolchain_dep(providers = [OciToolchainInfo]),
    "_run": attrs.default_only(attrs.source(default = "prelude//oci:run")),
    "_closure": attrs.default_only(attrs.source(default = "prelude//oci:closure")),
}, doc = "Runs an offline native command on a rootful Linux worker with immutable inputs at /inputs; preserves image defaults.")

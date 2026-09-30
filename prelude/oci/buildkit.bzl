# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Runs an explicitly selected private BuildKit worker over a declared context.

load(":providers.bzl", "OciImageInfo", "OciToolchainInfo", "oci_platform")
load("@prelude//:artifacts.bzl", "single_artifact")

def _managed_buildkit_impl(ctx: AnalysisContext) -> list[Provider]:
    """Track the selected launcher, runtime, client, daemon contract and image pin."""
    if "@sha256:" not in ctx.attrs.image:
        fail("managed_buildkit image requires a repository digest")
    return [DefaultInfo(), RunInfo(args = cmd_args(
        single_artifact(ctx.attrs.node).default_output, ctx.attrs._worker,
        "--docker", ctx.attrs.docker, "--daemon-contract", ctx.attrs.daemon_contract,
        "--buildkit-image", ctx.attrs.image, hidden = [ctx.attrs._operations, ctx.attrs._closure],
    ))]

managed_buildkit = rule(impl = _managed_buildkit_impl, attrs = {
    "node": attrs.exec_dep(),
    "docker": attrs.source(doc = "Pinned Docker client executable, not a PATH lookup."),
    "daemon_contract": attrs.source(doc = "Local daemon socket/version/API/kernel/architecture contract JSON."),
    "image": attrs.string(doc = "Already acquired BuildKit repository@sha256 digest."),
    "_worker": attrs.source(default = "prelude//oci:worker"),
    "_operations": attrs.source(default = "prelude//oci:operations"),
    "_closure": attrs.source(default = "prelude//oci:closure"),
}, doc = "Creates a closed-input private BuildKit launcher; each executed solve starts and removes its own isolated state.")

def _dockerfile_path(value: str) -> None:
    """Require a context-relative Dockerfile path that BuildKit can resolve safely."""
    if value == "" or value.startswith("/") or "\\" in value or ":" in value:
        fail("dockerfile_image dockerfile must be a normalized context-relative path")
    if "\x00" in value or "\n" in value or "\r" in value:
        fail("dockerfile_image dockerfile contains a control delimiter")
    for part in value.split("/"):
        if part in ["", ".", ".."]:
            fail("dockerfile_image dockerfile must be a normalized context-relative path")

def _dockerfile_image_impl(ctx: AnalysisContext) -> list[Provider]:
    """Solve a closed-input Dockerfile with managed tooling, then check its image."""
    platform = oci_platform(ctx.attrs.platform)
    _dockerfile_path(ctx.attrs.dockerfile)
    if ctx.attrs.source_date_epoch < 0:
        fail("dockerfile_image source_date_epoch must be nonnegative")
    for key in ctx.attrs.build_args:
        if key == "" or "=" in key or "\x00" in key or "\n" in key or "\r" in key:
            fail("dockerfile_image build argument name contains a delimiter")
        if key == "SOURCE_DATE_EPOCH":
            fail("dockerfile_image SOURCE_DATE_EPOCH is owned by source_date_epoch")
    if ctx.attrs.target != None and ctx.attrs.target == "":
        fail("dockerfile_image target must be nonempty when selected")
    spec = ctx.actions.write_json("dockerfile-inputs.json", {
        "context": ctx.attrs.context,
        "dockerfile": ctx.attrs.dockerfile,
        "platform": platform,
        "build_args": ctx.attrs.build_args,
        "target": ctx.attrs.target,
        "source_date_epoch": ctx.attrs.source_date_epoch,
    }, with_inputs = True)
    layout = ctx.actions.declare_output("buildkit-layout", dir = True)
    # The RunInfo must track the complete launcher/worker contract, not just an
    # ambient socket or version. Its worker state is fresh per executed solve;
    # BuildKit cache mounts therefore cannot introduce undeclared persistent data.
    # The qualified launcher uses embedded dockerfile.v0, allows only local://
    # acquisitions, disables networking and passes no privileged entitlements.
    ctx.actions.run(cmd_args(
        ctx.attrs.builder[RunInfo], "--spec", spec, "--output", layout.as_output(),
    ), category = "oci_buildkit", local_only = True, allow_cache_upload = True)
    manifest = ctx.actions.declare_output("manifest.json")
    config = ctx.actions.declare_output("config.json")
    descriptor = ctx.actions.declare_output("descriptor.json")
    toolchain = ctx.attrs.toolchain[OciToolchainInfo]
    ctx.actions.run(cmd_args(
        toolchain.operations, "import", "--layout", layout, "--platform", platform,
        "--manifest", manifest.as_output(), "--config", config.as_output(),
        "--descriptor", descriptor.as_output(),
    ), category = "oci_import", allow_cache_upload = True)
    return [
        DefaultInfo(default_output = descriptor, sub_targets = {
            "manifest": [DefaultInfo(default_output = manifest)],
            "config": [DefaultInfo(default_output = config)],
            "layout": [DefaultInfo(default_output = layout)],
        }),
        OciImageInfo(
            manifest = manifest,
            config = config,
            descriptor = descriptor,
            platform = platform,
            layers = [],
            layouts = [layout],
        ),
    ]

dockerfile_image = rule(
    impl = _dockerfile_image_impl,
    attrs = {
        "context": attrs.source(allow_directory = True, doc = "Declared directory containing the Dockerfile, ignore files and all local inputs."),
        "dockerfile": attrs.string(default = "Dockerfile", doc = "Context-relative filename passed unchanged to the embedded Dockerfile frontend."),
        "builder": attrs.exec_dep(providers = [RunInfo], doc = "Managed private-worker launcher accepting --spec and --output; its immutable tooling/configuration must be tracked inputs."),
        "platform": attrs.string(),
        "build_args": attrs.dict(key = attrs.string(), value = attrs.string(), default = {}),
        "target": attrs.option(attrs.string(), default = None),
        "source_date_epoch": attrs.int(default = 0),
        "toolchain": attrs.toolchain_dep(providers = [OciToolchainInfo]),
    },
    doc = "Builds a scratch/context-only Dockerfile using an explicitly selected private BuildKit worker and returns checked OCI metadata. External acquisitions fail; BuildKit owns each executed solve and BSMR owns outer action reuse.",
)

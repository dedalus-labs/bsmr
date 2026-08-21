# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Defines native Cargo workspace and build actions for inferred Rust targets.

CargoWorkspaceInfo = provider(fields = {
    "root": provider_field(Artifact),
    "toolchain": provider_field(str),
})

def _validate_workspace_path(path: str) -> None:
    """Reject non-normalized and build-system-owned Cargo input paths."""
    components = path.replace("\\", "/").split("/")
    if path == "" or path.startswith("/") or (len(path) > 1 and path[1] == ":"):
        fail("Cargo workspace input path '{}' must be relative".format(path))
    if "" in components or "." in components or ".." in components:
        fail("Cargo workspace input path '{}' must be normalized".format(path))
    if components[0] in [".bsmr", ".bsmr.local", ".git", "bsmr-out", "target"]:
        fail("Cargo workspace input path '{}' is owned by BSMR or Cargo".format(path))

def _cargo_workspace_impl(ctx: AnalysisContext) -> list[Provider]:
    """Materialize the exact source tree shared by native Cargo actions."""
    for path in ctx.attrs.srcs:
        _validate_workspace_path(path)
    root = ctx.actions.symlinked_dir(
        ctx.label.name,
        ctx.attrs.srcs,
        has_content_based_path = False,
    )
    return [
        DefaultInfo(default_output = root),
        CargoWorkspaceInfo(root = root, toolchain = ctx.attrs.toolchain),
    ]

cargo_workspace = rule(
    impl = _cargo_workspace_impl,
    attrs = {
        "srcs": attrs.dict(attrs.string(), attrs.source()),
        "toolchain": attrs.string(),
    },
    doc = "Defines the source tree and exact rustup channel for one Cargo workspace.",
)

def _cargo_build_impl(ctx: AnalysisContext) -> list[Provider]:
    """Run Cargo with isolated dependency state and cache its complete target directory."""
    _validate_workspace_path(ctx.attrs.manifest)
    workspace = ctx.attrs.workspace[CargoWorkspaceInfo]
    manifest = workspace.root.project(ctx.attrs.manifest)
    cargo_home = ctx.actions.declare_output("__{}_cargo_home".format(ctx.label.name), dir = True, has_content_based_path = True)
    target = ctx.actions.declare_output(ctx.label.name, dir = True, has_content_based_path = True)
    remap_flags = cmd_args(
        [
            cmd_args(workspace.root, format = "--remap-path-prefix={}=/bsmr/workspace"),
            cmd_args(cargo_home.as_output(), format = "--remap-path-prefix={}=/bsmr/cargo-home"),
        ],
        delimiter = " ",
    )
    ctx.actions.run(
        cmd_args([
            "cargo",
            "build",
            "--locked",
            "--manifest-path",
            manifest,
        ]),
        env = {
            "CARGO_HOME": cargo_home.as_output(),
            "CARGO_INCREMENTAL": "0",
            "CARGO_TARGET_DIR": target.as_output(),
            "CARGO_TERM_COLOR": "never",
            "RUSTFLAGS": remap_flags,
            "RUSTUP_TOOLCHAIN": workspace.toolchain,
        },
        allow_cache_upload = False,
        category = "cargo_build",
        identifier = ctx.label.name,
        local_only = True,
    )
    return [DefaultInfo(default_output = target, other_outputs = [cargo_home])]

cargo_build = rule(
    impl = _cargo_build_impl,
    attrs = {
        "manifest": attrs.string(),
        "workspace": attrs.dep(providers = [CargoWorkspaceInfo]),
    },
    doc = "Builds one Cargo manifest with locked dependencies and isolated mutable state.",
)

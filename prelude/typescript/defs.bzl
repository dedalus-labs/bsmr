# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Defines cache-separable TypeScript source, typecheck, and library actions.

load("@prelude//toolchains/pnpm:defs.bzl", "PnpmInstallInfo")

TypescriptSourcesInfo = provider(fields = {
    "files": provider_field(dict[str, Artifact]),
})

def _validate_workspace_path(path: str) -> None:
    """Reject non-normalized and build-system-owned workspace paths."""
    components = path.replace("\\", "/").split("/")
    if path == "" or path.startswith("/") or (len(path) > 1 and path[1] == ":"):
        fail("TypeScript source path '{}' must be relative".format(path))
    if "" in components or "." in components or ".." in components:
        fail("TypeScript source path '{}' must be normalized".format(path))
    if components[0] == ".bsmr":
        fail("TypeScript sources may not use BSMR's reserved '.bsmr' path")

def _typescript_sources_impl(ctx: AnalysisContext) -> list[Provider]:
    """Merge one package and its exact workspace dependency closure."""
    files = {}
    for dependency in ctx.attrs.deps:
        for path, source in dependency[TypescriptSourcesInfo].files.items():
            if path in files and files[path] != source:
                fail("TypeScript dependency closure contains conflicting path '{}'".format(path))
            files[path] = source
    for path, source in ctx.attrs.srcs.items():
        _validate_workspace_path(path)
        if path in files and files[path] != source:
            fail("TypeScript package source conflicts with dependency path '{}'".format(path))
        files[path] = source
    tree = ctx.actions.symlinked_dir(
        ctx.label.name,
        files,
        has_content_based_path = False,
    )
    return [
        DefaultInfo(default_output = tree),
        TypescriptSourcesInfo(files = files),
    ]

typescript_sources = rule(
    impl = _typescript_sources_impl,
    attrs = {
        "deps": attrs.list(attrs.dep(providers = [TypescriptSourcesInfo]), default = []),
        "srcs": attrs.dict(
            attrs.string(),
            attrs.source(),
            doc = "Workspace-relative source paths and their owning artifacts.",
        ),
    },
    doc = "Defines one TypeScript package's transitive source closure without running a tool.",
)

def _require_declared_config(files: dict[str, Artifact], package_root: str, config: str) -> None:
    """Require the action's package manifest and selected tool configuration."""
    for path in [
        "package.json" if package_root == "." else "{}/package.json".format(package_root),
        config if package_root == "." else "{}/{}".format(package_root, config),
    ]:
        if path not in files:
            fail("TypeScript action source closure does not declare '{}'".format(path))

def _run_typescript(ctx: AnalysisContext, mode: str, output: Artifact) -> None:
    """Register one TypeScript action over a frozen install and source closure."""
    sources = ctx.attrs.sources[TypescriptSourcesInfo]
    if ctx.attrs.package_root != ".":
        _validate_workspace_path(ctx.attrs.package_root)
    _validate_workspace_path(ctx.attrs.config)
    _require_declared_config(sources.files, ctx.attrs.package_root, ctx.attrs.config)
    source_tree = ctx.actions.symlinked_dir(
        "__{}_typescript_srcs__".format(ctx.label.name),
        sources.files,
        has_content_based_path = False,
    )
    install = ctx.attrs.install[PnpmInstallInfo]
    command = cmd_args([
        install.node,
        ctx.attrs._runner,
        "--config",
        ctx.attrs.config,
        "--install",
        install.workspace,
        "--mode",
        mode,
        "--output",
        output.as_output(),
        "--package-root",
        ctx.attrs.package_root,
        "--source",
        source_tree,
    ])
    ctx.actions.run(
        command,
        allow_cache_upload = True,
        category = "typescript_{}".format(mode),
        identifier = ctx.label.name,
    )

def _typescript_typecheck_impl(ctx: AnalysisContext) -> list[Provider]:
    """Run the package-local locked TypeScript compiler without emission."""
    output = ctx.actions.declare_output("{}.typecheck".format(ctx.label.name))
    _run_typescript(ctx, "typecheck", output)
    return [DefaultInfo(default_output = output)]

def _typescript_library_impl(ctx: AnalysisContext) -> list[Provider]:
    """Run the package-local locked tsdown compiler into a cached directory."""
    output = ctx.actions.declare_output(ctx.label.name, dir = True, has_content_based_path = True)
    _run_typescript(ctx, "library", output)
    return [DefaultInfo(default_output = output)]

def _typescript_action_attrs(default_config: str) -> dict:
    """Return the shared closed attribute schema for TypeScript actions."""
    return {
        "config": attrs.string(default = default_config),
        "install": attrs.dep(providers = [PnpmInstallInfo]),
        "package_root": attrs.string(),
        "sources": attrs.dep(providers = [TypescriptSourcesInfo]),
        "_runner": attrs.source(default = "prelude//typescript:runner"),
    }

typescript_typecheck = rule(
    impl = _typescript_typecheck_impl,
    attrs = _typescript_action_attrs("tsconfig.json"),
    doc = "Typechecks one pnpm workspace package with its exact locked TypeScript compiler.",
)

typescript_library = rule(
    impl = _typescript_library_impl,
    attrs = _typescript_action_attrs("tsdown.config.ts"),
    doc = "Emits one pnpm workspace package with its exact locked tsdown compiler.",
)

# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Assembles the declared workspace without exposing Git configuration or hooks.

# Paths are relative to the package that provides these immutable source artifacts.
CheckoutSources = provider(fields = {
    "files": provider_field(dict[str, Artifact]),
    "unavailable": provider_field(list[str], default = []),
})

def _impl(ctx: AnalysisContext) -> list[Provider]:
    """Keep the real HEAD, index, refs and objects beside the captured working files."""
    files = {path: package[DefaultInfo].default_outputs[0] for path, package in ctx.attrs.packages.items()}
    declared = {
        (path + "/" if path else "") + name: source
        for path, package in ctx.attrs.packages.items()
        for name, source in package[CheckoutSources].files.items()
    }
    if ctx.attrs.git:
        # Git requires refs/ even when every reference lives in packed-refs.
        files[".git/refs"] = ctx.actions.copied_dir("refs", {}, has_content_based_path = True)
    for path, (url, sha256, size) in ctx.attrs.git.items():
        files[path] = ctx.actions.download_file(path, url, sha256 = sha256, size_bytes = size, has_content_based_path = True)
    if ctx.attrs.git:
        files[".git/config"] = ctx.actions.write("config", "[core]\nrepositoryformatversion=0\nbare=false\n")
    root = ctx.actions.copied_dir("checkout", files, symlinks = "preserve", has_content_based_path = True)
    return [DefaultInfo(default_output = root), CheckoutSources(files = declared, unavailable = ctx.attrs.unavailable)]

cargo_checkout = rule(
    impl = _impl,
    attrs = {
        "packages": attrs.dict(attrs.string(), attrs.dep(providers = [CheckoutSources])),
        "git": attrs.dict(attrs.string(), attrs.tuple(attrs.string(), attrs.string(), attrs.int())),
        "unavailable": attrs.list(attrs.string()),
    },
)

def _source(ctx: AnalysisContext) -> list[Provider]:
    """Keep the package, ancestor files and declared extras at their original paths."""
    package = ctx.attrs.package
    boundaries = {path: True for path in ctx.attrs.boundaries}
    declared = ctx.attrs.checkout[CheckoutSources].files
    sources = {}
    for path, source in declared.items():
        owner = ""
        parts = path.split("/")
        for end in range(1, len(parts)):
            prefix = "/".join(parts[:end])
            if prefix in boundaries:
                owner = prefix
        if not owner or owner == package or package.startswith(owner + "/"):
            sources[path] = source
    for path in ctx.attrs.extra_srcs:
        if path not in declared:
            fail("extra source {} is not a declared workspace file".format(path))
        sources[path] = declared[path]
    root = ctx.actions.copied_dir("source", sources, symlinks = "preserve", has_content_based_path = True)
    return [DefaultInfo(
        default_output = root,
        sub_targets = {"package": [DefaultInfo(default_output = root.project(package))]},
    )]

_cargo_source = rule(
    impl = _source,
    attrs = {
        "checkout": attrs.dep(providers = [CheckoutSources]),
        "package": attrs.string(),
        "boundaries": attrs.list(attrs.string(), doc = "Cargo-declared package paths relative to the workspace."),
        "extra_srcs": attrs.list(attrs.string(), doc = "Workspace-relative files declared by the consuming package."),
    },
)

def cargo_source(name: str, package: str, **kwargs):
    """Track explicit cross-package files through the caller's native configuration."""
    extra = json.decode(read_config("rust.sources", package or ".", "[]"))
    _cargo_source(name = name, package = package, extra_srcs = extra, **kwargs)

def _files(ctx: AnalysisContext) -> list[Provider]:
    """Keep each package's source bytes and relative links until workspace assembly."""
    empty = ctx.actions.copied_dir("empty", {}, has_content_based_path = True)
    sources = {path: empty for path in ctx.attrs.directories}
    sources.update({source.short_path: source for source in ctx.attrs.srcs})
    root = ctx.actions.copied_dir("files", sources, symlinks = "preserve", has_content_based_path = True)
    return [DefaultInfo(default_output = root), CheckoutSources(files = sources)]

checkout_files = rule(
    impl = _files,
    attrs = {"srcs": attrs.list(attrs.source()), "directories": attrs.list(attrs.string(), default = [])},
)

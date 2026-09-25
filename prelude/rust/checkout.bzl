# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Assembles the declared workspace without exposing Git configuration or hooks.

def _impl(ctx: AnalysisContext) -> list[Provider]:
    """Keep the real HEAD, index, refs and objects beside the captured working files."""
    if ctx.attrs.unavailable:
        fail("workspace script inputs require native source packages: {}".format(ctx.attrs.unavailable))
    files = dict(ctx.attrs.packages)
    if ctx.attrs.git:
        # Git requires refs/ even when every reference lives in packed-refs.
        files[".git/refs"] = ctx.actions.copied_dir("refs", {}, has_content_based_path = True)
    for path, (url, sha256, size) in ctx.attrs.git.items():
        files[path] = ctx.actions.download_file(path, url, sha256 = sha256, size_bytes = size, has_content_based_path = True)
    if ctx.attrs.git:
        files[".git/config"] = ctx.actions.write("config", "[core]\nrepositoryformatversion=0\nbare=false\n")
    root = ctx.actions.copied_dir("checkout", files, symlinks = "preserve", has_content_based_path = True)
    return [DefaultInfo(default_output = root)]

cargo_checkout = rule(
    impl = _impl,
    attrs = {
        "packages": attrs.dict(attrs.string(), attrs.source(allow_directory = True)),
        "git": attrs.dict(attrs.string(), attrs.tuple(attrs.string(), attrs.string(), attrs.int())),
        "unavailable": attrs.list(attrs.string()),
    },
)

def _files(ctx: AnalysisContext) -> list[Provider]:
    """Keep each package's source bytes and relative links until workspace assembly."""
    empty = ctx.actions.copied_dir("empty", {}, has_content_based_path = True)
    sources = {path: empty for path in ctx.attrs.directories}
    sources.update({source.short_path: source for source in ctx.attrs.srcs})
    root = ctx.actions.copied_dir("files", sources, symlinks = "preserve", has_content_based_path = True)
    return [DefaultInfo(default_output = root)]

checkout_files = rule(
    impl = _files,
    attrs = {"srcs": attrs.list(attrs.source()), "directories": attrs.list(attrs.string(), default = [])},
)

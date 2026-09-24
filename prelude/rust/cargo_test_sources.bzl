# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Stages selected workspace packages in their original relative layout for tests.

def _impl(ctx: AnalysisContext) -> list[Provider]:
    """Expose the full fixture tree and its selected package through native artifacts."""
    root = ctx.actions.copied_dir("workspace", ctx.attrs.packages, has_content_based_path = True)
    return [DefaultInfo(
        default_output = root,
        sub_targets = {"package": [DefaultInfo(default_output = root.project(ctx.attrs.package))]},
    )]

cargo_test_sources = rule(
    impl = _impl,
    attrs = {
        "packages": attrs.dict(attrs.string(), attrs.source(allow_directory = True), doc = "Workspace-relative package directories and their declared source trees."),
        "package": attrs.string(doc = "Selected package's relative workspace path, empty at the workspace root."),
    },
)

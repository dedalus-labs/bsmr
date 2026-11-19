# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Materializes every output requested by a Cargo graph while retaining native providers.

def _cargo_outputs_impl(ctx: AnalysisContext) -> list[Provider]:
    """Preserve the root's providers and require all declared companion artifacts."""
    actual = ctx.attrs.actual
    if actual == None:
        return [DefaultInfo()]
    default = actual[DefaultInfo]
    sub_targets = {name: actual.sub_target(name).providers for name in default.sub_targets}
    if "staticlib_pic" in sub_targets:
        sub_targets["staticlib"] = sub_targets["staticlib_pic"]
    return [provider for provider in actual.providers if not isinstance(provider, DefaultInfo)] + [
        DefaultInfo(
            default_outputs = default.default_outputs,
            other_outputs = default.other_outputs + ctx.attrs.outputs,
            sub_targets = sub_targets,
        ),
    ]

cargo_outputs = rule(
    impl = _cargo_outputs_impl,
    attrs = {
        "actual": attrs.option(attrs.dep(), default = None),
        "outputs": attrs.list(attrs.source()),
    },
)

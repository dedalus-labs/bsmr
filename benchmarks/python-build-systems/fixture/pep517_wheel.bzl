# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Defines the semantics-matched Bazel PEP 517 wheel control.

def _pep517_wheel_impl(ctx):
    """Runs one declared PEP 517 backend over an immutable source copy."""
    output = ctx.actions.declare_directory(ctx.label.name)
    builder = ctx.attr.builder[DefaultInfo].files_to_run
    arguments = ctx.actions.args()
    arguments.add("--output", output.path)
    arguments.add_all(ctx.files.srcs, before_each = "--source")
    arguments.use_param_file("@%s", use_always = True)
    ctx.actions.run(
        arguments = [arguments],
        env = {"SOURCE_DATE_EPOCH": "315532800"},
        executable = builder,
        inputs = depset(ctx.files.srcs),
        mnemonic = "Pep517Wheel",
        outputs = [output],
        tools = [builder],
    )
    return [DefaultInfo(files = depset([output]))]

pep517_wheel = rule(
    implementation = _pep517_wheel_impl,
    attrs = {
        "builder": attr.label(executable = True, cfg = "exec", mandatory = True),
        "srcs": attr.label_list(allow_files = True, mandatory = True),
    },
)

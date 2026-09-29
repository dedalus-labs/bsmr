# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

def _touch_file_impl(ctx):
    if ctx.attrs.out != None:
        out = ctx.actions.write(ctx.attrs.out, "", has_content_based_path = False)
        default_outputs = [out]
        named_outputs = {}
    elif ctx.attrs.outs != None:
        default_outputs = []
        named_outputs = {}
        default_out_paths = ctx.attrs.default_outs or []
        for name, path in ctx.attrs.outs.items():
            artifact = ctx.actions.write(path, "", has_content_based_path = False)
            if path in default_out_paths:
                default_outputs.append(artifact)
            named_outputs[name] = artifact
    else:
        fail("One of `out` or `outs` should be set.")
    providers = [
        DefaultInfo(
            default_outputs = default_outputs,
            sub_targets = {k: [DefaultInfo(default_output = v)] for (k, v) in named_outputs.items()},
        )
    ]
    return providers

def _mkdir_impl(ctx):
    out = ctx.actions.declare_output("out", dir = True, has_content_based_path = False)
    ctx.actions.run(
        cmd_args(
            "fbpython",
            "-c",
            """
import sys
import os

f = sys.argv[1]
os.makedirs(f + "/nested/empty")
with open(f + "/hello", "w") as out:
    out.write("hello")
with open(f + "/nested/executable", "w") as out:
    out.write("#!/bin/sh")
os.chmod(f + "/nested/executable", 0o755)
os.symlink("../hello", f + "/nested/relative-link")
os.symlink("/dev/null", f + "/nested/external-link")
""",
            out.as_output(),
        ),
        category = "create_dir",
    )
    return [DefaultInfo(out)]

def _cacheable_outputs_impl(ctx):
    outputs = [ctx.actions.declare_output(name, has_content_based_path = False) for name in ["one.txt", "two.txt"]]
    ctx.actions.run(
        cmd_args(
            "fbpython",
            "-c",
            "import pathlib, sys; [pathlib.Path(path).write_text(path) for path in sys.argv[1:]]",
            [output.as_output() for output in outputs],
        ),
        category = "cacheable_outputs",
        allow_cache_upload = True,
    )
    return [DefaultInfo(default_outputs = outputs)]

def _source_output_impl(ctx):
    return [DefaultInfo(default_output = ctx.attrs.src)]

touch_file = rule(
    impl = _touch_file_impl,
    attrs = {
        "default_outs": attrs.option(attrs.set(attrs.string(), sorted = False), default = None),
        "deps": attrs.list(attrs.dep(), default = []),
        "out": attrs.option(attrs.string(), default = None),
        "outs": attrs.option(attrs.dict(key = attrs.string(), value = attrs.string(), sorted = False), default = None),
    },
)

mkdir = rule(impl = _mkdir_impl, attrs = {})

cacheable_outputs = rule(impl = _cacheable_outputs_impl, attrs = {})

source_output = rule(
    impl = _source_output_impl,
    attrs = {"src": attrs.source()},
)

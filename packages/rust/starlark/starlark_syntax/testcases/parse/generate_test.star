# ===----------------------------------------------------------------------===
# Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc
# Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# @generated
def _generate_script_impl(ctx):
  script_file = ctx.actions.declare_file(ctx.label.name + ".bash")
  ctx.actions.write(output=script_file, is_executable=True, content="""
{0}
""".format(ctx.file.binary.short_path))
  return struct(
      files = depset([script_file]),
  )


generate_script = rule(
    _generate_script_impl,
    attrs = {
        "binary": attr.label(allow_files=True, single_file=True),
    },
)

# ===----------------------------------------------------------------------===
# Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc
# Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# @generated
# Label of the template file to use.
_TEMPLATE = "//expand_template:hello.cc"

def _hello_impl(ctx):
  ctx.actions.expand_template(
      template=ctx.file._template,
      output=ctx.outputs.source_file,
      substitutions={
          "{FIRSTNAME}": ctx.attr.firstname
      })

hello = rule(
    implementation=_hello_impl,
    attrs={
        "firstname": attr.string(mandatory=True),
        "_template": attr.label(
            default=Label(_TEMPLATE), allow_files=True, single_file=True),
    },
    outputs={"source_file": "%{name}.cc"},
)

# ===----------------------------------------------------------------------===
# Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# @generated
"""Minimalist example of a rule that does nothing."""

def _empty_impl(ctx):
  # This function is called when the rule is analyzed.
  # You may use print for debugging.
  print("This rule does nothing")

empty = rule(implementation=_empty_impl)

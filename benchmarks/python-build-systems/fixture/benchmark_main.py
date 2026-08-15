# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Exercises the Django console entry point built by each system.

"""Exercise the Django console entry point built by each system."""

import re
from pathlib import Path

import django
from django.views.generic import base

source = Path(base.__file__).read_text(encoding="utf-8")
assert (
    len(re.findall(r"^# BSMR benchmark leaf [a-z]+-\d+\.$", source, re.MULTILINE)) == 1
)

print(django.get_version())

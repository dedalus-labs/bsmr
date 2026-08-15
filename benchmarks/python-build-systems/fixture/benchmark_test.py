# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Verifies the Django import closure through each test runner.

"""Verify the Django import closure through each test runner."""

import re
from pathlib import Path

import django
from django.conf import global_settings
from django.views.generic import base

assert django.get_version().startswith("6.2")
assert global_settings.DEFAULT_CHARSET == "utf-8"
source = Path(base.__file__).read_text(encoding="utf-8")
assert (
    len(re.findall(r"^# BSMR benchmark leaf [a-z]+-\d+\.$", source, re.MULTILINE)) == 1
)

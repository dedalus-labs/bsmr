# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Verify the Django import closure through each test runner."""

import django
from django.conf import global_settings


assert django.get_version().startswith("6.2")
assert global_settings.DEFAULT_CHARSET == "utf-8"

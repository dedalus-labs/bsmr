#!/bin/bash
# ===----------------------------------------------------------------------===
# Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc
# Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

set -e

dnf download "$1" --destdir "$BSMR_SCRATCH_PATH"
rpm=$(echo "$BSMR_SCRATCH_PATH"/*)
mkdir -p "$2"
rpm2archive - < "$rpm" | tar -xvzf - -C "$(realpath "$2")"

if [[ $1 =~ fish ]]; then
    # In order to get fish to behave like it's been installed into a relocatable
    # directory, we need to move things out of `usr/`
    mv "$2/usr/"* "$2"
    rmdir "$2/usr"
fi

# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Rejects an active network connection outside the loopback interface.

set -eu

# Linux can create inactive tunnel devices in a fresh network namespace.
# IFF_UP=1 and IFF_LOOPBACK=8 are the kernel's interface flags.
for flags_path in /sys/class/net/*/flags; do
    read -r flags < "$flags_path"
    if [ "$((flags & 9))" -eq 1 ]; then
        printf 'network interface is up: %s\n' "${flags_path%/flags}" >&2
        exit 1
    fi
done

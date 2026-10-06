#!/bin/bash
# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Reject an active network connection outside loopback. Native OCI tests run
# this script inside the image; the runtime must mount Linux interface state.

set -euo pipefail

# Linux exposes one flags file per interface, including inactive tunnel devices.
readonly INTERFACE_DIRECTORY=/sys/class/net
# These kernel flags identify an active interface and the permitted loopback device.
readonly IFF_UP=1
readonly IFF_LOOPBACK=8

# Inactive devices are harmless; an active non-loopback device violates isolation.
for flags_path in "$INTERFACE_DIRECTORY"/*/flags; do
  read -r flags < "$flags_path"
  if (( (flags & (IFF_UP | IFF_LOOPBACK)) == IFF_UP )); then
    printf 'network interface is up: %s\n' "${flags_path%/flags}" >&2
    exit 1
  fi
done

#!/bin/bash
# Append a receipt after installation. The native OCI graph adds this script to
# the install command to prove that a command-only change invalidates the cache.
set -euo pipefail

# The graph checks this separate output to distinguish command and input changes.
readonly COMMAND_MARKER=/command-marker

# A warm result from the previous command cannot contain this receipt.
printf '%s' command > "$COMMAND_MARKER"

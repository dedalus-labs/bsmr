#!/bin/bash
# Supply an executable file for the compact-layer graph's directory input.
# The graph inspects its mode and bytes; the file accepts no arguments.
set -euo pipefail

# Keep the executable harmless if invoked while inspecting the exported image.
exit 0

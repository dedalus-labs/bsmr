#!/bin/bash
# Install locked packages and retain the input marker. The native OCI graph runs
# this script inside its Debian image with read-only files mounted under /inputs.
#
#   DEBIAN_FRONTEND  Must be noninteractive so package installation needs no terminal.
set -euo pipefail

: "${DEBIAN_FRONTEND:?set DEBIAN_FRONTEND for unattended installation}"

# The rule mounts its package closure and marker at these fixed fixture paths.
readonly PACKAGE_DIRECTORY=/inputs/packages
readonly INPUT_MARKER=/inputs/marker
readonly IMAGE_MARKER=/image-marker

# Disable repository discovery; only the declared package closure may be installed.
readonly -a APT_OPTIONS=(
  -o Dir::Etc::sourcelist=/dev/null
  -o Dir::Etc::sourceparts=-
  -y --no-install-recommends
)

# Exercise real package installation, including each package's post-install script.
apt-get "${APT_OPTIONS[@]}" install "$PACKAGE_DIRECTORY"/*.deb

# Preserve the input marker so the graph can prove that changed inputs rerun this step.
cp "$INPUT_MARKER" "$IMAGE_MARKER"

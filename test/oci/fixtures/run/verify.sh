#!/bin/bash
# Verify the installed tools, certificates, and network isolation. The native OCI
# graph runs this script in the image produced by the package-install step.
set -euo pipefail

# Package post-install scripts must populate the standard Debian certificate bundle.
readonly CERTIFICATE_BUNDLE=/etc/ssl/certs/ca-certificates.crt
# The rule mounts the network-isolation check as a read-only input.
readonly OFFLINE_CHECK=/inputs/offline.sh
# These are the packages requested by the fixture's lock file.
readonly -a PACKAGES=(ca-certificates curl)

# Require a working curl executable and certificates before publishing the image.
curl --version
if [[ ! -s "$CERTIFICATE_BUNDLE" ]]; then
  printf 'certificate bundle is missing or empty: %s\n' "$CERTIFICATE_BUNDLE" >&2
  exit 1
fi
dpkg-query -W "${PACKAGES[@]}"

# Installation must not grant the next step access to an external network.
/bin/bash "$OFFLINE_CHECK"

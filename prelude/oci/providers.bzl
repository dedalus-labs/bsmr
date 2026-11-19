# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Carries OCI metadata independently of the content needed for complete exports.

OciLayerInfo = provider(
    doc = "An ordinary compressed layer and its img metadata for one Linux platform.",
    fields = {
        "metadata": provider_field(Artifact),
        "blob": provider_field(Artifact),
        "platform": provider_field(str),
    },
)

OciImageInfo = provider(
    doc = "An image's small metadata and deferred immutable content closure.",
    fields = {
        "manifest": provider_field(Artifact),
        "config": provider_field(Artifact),
        "descriptor": provider_field(Artifact),
        "platform": provider_field(str),
        "layers": provider_field(list[OciLayerInfo]),
        "layouts": provider_field(list[Artifact], default = []),
    },
)

OciToolchainInfo = provider(
    doc = "Pinned img executable and the checked OCI operations on the execution platform.",
    fields = {
        "img": provider_field(Artifact),
        "operations": provider_field(RunInfo),
    },
)

def oci_platform(value: str) -> str:
    """Normalize qualified Linux platforms without inferring the build host."""
    if value == "linux/amd64":
        return value
    if value in ["linux/arm64", "linux/arm64/v8"]:
        return "linux/arm64"
    fail("OCI platform must be linux/amd64 or linux/arm64[/v8], got '{}'".format(value))

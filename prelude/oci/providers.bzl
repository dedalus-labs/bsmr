# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Carries OCI metadata independently of the content needed for complete exports.

OciLayerInfo = provider(
    doc = "Layer metadata with either a retained archive or compact stream and original inputs.",
    fields = {
        "metadata": provider_field(Artifact),
        "blob": provider_field(Artifact | None, default = None),
        "compact": provider_field(Artifact | None, default = None),
        "inputs": provider_field(WriteJsonCliArgs | None, default = None),
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
        "node": provider_field(Artifact),
        "umoci": provider_field(Artifact | None, default = None),
        "runc": provider_field(Artifact | None, default = None),
        "operations": provider_field(RunInfo),
    },
)

def oci_layout_spec(image: OciImageInfo) -> dict:
    """Carry every retained payload into complete exports and filesystem actions."""
    return {
        "manifest": image.manifest,
        "config": image.config,
        "descriptor": image.descriptor,
        "layers": [
            {"metadata": layer.metadata, "blob": layer.blob} if layer.blob != None else
            {"metadata": layer.metadata, "compact": layer.compact, "inputs": cmd_args(layer.inputs, delimiter = "")}
            for layer in image.layers
        ],
        "base_layouts": image.layouts,
    }

def oci_platform(value: str) -> str:
    """Normalize qualified Linux platforms without inferring the build host."""
    if value == "linux/amd64":
        return value
    if value in ["linux/arm64", "linux/arm64/v8"]:
        return "linux/arm64"
    fail("OCI platform must be linux/amd64 or linux/arm64[/v8], got '{}'".format(value))

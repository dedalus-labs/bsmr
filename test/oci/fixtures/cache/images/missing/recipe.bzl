load("@prelude//oci:defs.bzl", "oci_layer")
load(":fixture.json", fixture="value")

oci_layer(
    name="layer",
    platform=fixture["platform"],
    files={"/app/missing": "missing.txt"},
    toolchain="//tools:oci",
)

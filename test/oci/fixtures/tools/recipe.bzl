load("@prelude//oci:defs.bzl", "oci_layer")
load("@prelude//oci:toolchain.bzl", "oci_toolchain")
load(":defs.bzl", "artifact")
load(":fixture.json", fixture="value")

artifact(name="imgtool", binary="img")
artifact(name="runtime", binary="node")
oci_toolchain(name="oci", img=":imgtool", node=":runtime")
oci_layer(
    name="layer", platform=fixture["platform"], files={"/fixture": "fixture-dir"}, toolchain=":oci"
)

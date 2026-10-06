load(
    "@prelude//oci:defs.bzl",
    "oci_layer",
    "oci_layer_from_tar",
    "oci_image",
    "oci_layout",
    "oci_push",
)
load("@prelude//oci:toolchain.bzl", "oci_toolchain")
load(":defs.bzl", "artifact")
load(":fixture.json", fixture="value")

artifact(name="imgtool", binary="img")
artifact(name="runtime", binary="node")
oci_toolchain(name="oci", img=":imgtool", node=":runtime")
export_file(name="payload", src="payload.txt", mode="copy")
oci_layer(
    name="layer", platform=fixture["platform"], files={"/app/payload": ":payload"}, toolchain=":oci"
)
oci_layer(
    name="tree_layer", platform=fixture["platform"], files={"/tree": "tree"}, toolchain=":oci"
)
oci_image(
    name="image",
    platform=fixture["platform"],
    layers=[":layer", ":tree_layer"],
    labels=fixture["labels"],
    toolchain=":oci",
)
oci_layout(name="layout", image=":image", toolchain=":oci")
oci_push(
    name="publish",
    image=":image",
    repository="example.invalid/team/image",
    tags=["test"],
    toolchain=":oci",
)
oci_push(
    name="publish_digest", image=":image", repository="example.invalid/team/image", toolchain=":oci"
)

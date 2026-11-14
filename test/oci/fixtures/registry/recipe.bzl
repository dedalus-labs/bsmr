load("@prelude//oci:defs.bzl", "oci_import", "oci_push")
load("@prelude//oci:toolchain.bzl", "oci_toolchain")
load(":artifact.bzl", "artifact")
load(":fixture.json", fixture="value")

artifact(name="encoder", binary="img")
artifact(name="runtime", binary="node")
oci_toolchain(name="oci", img=":encoder", node=":runtime", visibility=["PUBLIC"])
oci_import(name="base", layout="base", platform=fixture["platform"], toolchain=":oci")
oci_push(
    name="publish",
    image=":base",
    repository=fixture["repository"],
    tags=["proof"],
    toolchain=":oci",
)

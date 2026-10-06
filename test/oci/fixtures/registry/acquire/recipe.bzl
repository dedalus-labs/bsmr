load("@prelude//oci:defs.bzl", "oci_fetch", "oci_pull")
load(":image.lock.json", lock="value")

oci_fetch(
    name="base",
    image=lock["image"],
    platform=lock["platform"],
    lock="image.lock.json",
    toolchain="//:oci",
)
oci_pull(
    name="anonymous",
    image=lock["image"],
    platform=lock["platform"],
    lock="image.lock.json",
    toolchain="//:oci",
)

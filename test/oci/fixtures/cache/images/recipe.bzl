load("@prelude//oci:defs.bzl", "oci_layer", "oci_image", "oci_layout")
load(":fixture.json", fixture="value")

platform(
    name="linux_target",
    constraint_values=["config//os/constraints:linux", "config//cpu/constraints:" + fixture["cpu"]],
)
oci_layer(
    name="application",
    platform=fixture["platform"],
    executables={"/app/probe": "//cmd/probe:bin"},
    files={"/app/message.txt": "message.txt"},
    symlinks={"/app/current": "probe"},
    toolchain="//tools:oci",
)
oci_image(
    name="image",
    platform=fixture["platform"],
    layers=[":application"],
    entrypoint=["/app/current"],
    env=fixture["env"],
    user="65532:65532",
    working_dir="/app",
    toolchain="//tools:oci",
)
oci_layout(name="layout", image=":image", toolchain="//tools:oci")
oci_image(
    name="wrong_platform",
    platform=fixture["otherPlatform"],
    layers=[":application"],
    toolchain="//tools:oci",
)
oci_layer(
    name="unsafe_path",
    platform=fixture["platform"],
    files={"/app/../escape": "message.txt"},
    toolchain="//tools:oci",
)

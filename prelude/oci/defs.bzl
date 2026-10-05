# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Builds OCI metadata separately from immutable layer payloads and full exports.

load(":providers.bzl", "OciImageInfo", "OciLayerInfo", "OciToolchainInfo", "oci_layout_spec", "oci_platform")

load(":sources.bzl", _oci_pull = "oci_pull")

oci_pull = _oci_pull

def _path(value: str) -> None:
    """Require an absolute normalized image path without placement delimiters."""
    parts = value[1:].split("/")
    if not value.startswith("/") or "" in parts or "." in parts or ".." in parts:
        fail("OCI path must be absolute and normalized, got '{}'".format(value))
    if "=" in value or "\\" in value or "\x00" in value or "\n" in value or "\r" in value or value.strip() != value:
        fail("OCI path contains an unsupported placement delimiter: '{}'".format(value))
    for part in parts:
        if part.startswith(".wh."):
            fail("OCI whiteout names require a deletion rule, got '{}'".format(value))

def _placements(ctx: AnalysisContext):
    """Validate explicit destinations and retain every source as an action input."""
    paths = sorted(list(ctx.attrs.files) + list(ctx.attrs.executables) + list(ctx.attrs.symlinks))
    if not paths:
        fail("OCI placement requires at least one file, executable, or symlink")
    seen = {}
    for path in paths:
        _path(path)
        if path in seen:
            fail("OCI layer places more than one entry at '{}'".format(path))
        seen[path] = True
    for path in paths:
        ancestor = ""
        for component in path[1:].split("/")[:-1]:
            ancestor += "/" + component
            if ancestor in seen:
                fail("OCI entry '{}' has a placed file or symlink ancestor '{}'".format(path, ancestor))
    for target in ctx.attrs.symlinks.values():
        if target == "" or "\x00" in target or "\n" in target or "\r" in target:
            fail("OCI symlink target must be nonempty and contain no control delimiters")
    return ctx.actions.write_json("placements.json", {
        "files": ctx.attrs.files,
        "executables": ctx.attrs.executables,
        "symlinks": ctx.attrs.symlinks,
    }, with_inputs = True)

def _oci_layer_impl(ctx: AnalysisContext) -> list[Provider]:
    """Retain compact layer metadata and original payload inputs until export."""
    platform = oci_platform(ctx.attrs.platform)
    spec = _placements(ctx)
    metadata = ctx.actions.declare_output("layer.json")
    compact = ctx.actions.declare_output("layer.cstream")
    blob = ctx.actions.declare_output("layer.tgz")
    toolchain = ctx.attrs.toolchain[OciToolchainInfo]
    ctx.actions.run(cmd_args(
        toolchain.operations, "layer", "--img", toolchain.img, "--platform", platform, "--spec", spec,
        "--metadata", metadata.as_output(), "--compact", compact.as_output(),
    ), category = "oci_layer", allow_cache_upload = True)
    ctx.actions.run(cmd_args(
        toolchain.operations, "materialize", "--img", toolchain.img, "--platform", platform, "--spec", spec,
        "--metadata", metadata, "--compact", compact, "--blob", blob.as_output(),
    ), category = "oci_materialize", allow_cache_upload = False, allow_local_cache_upload = False)
    return [
        DefaultInfo(default_output = metadata, sub_targets = {
            "blob": [DefaultInfo(default_output = blob)],
            "compact": [DefaultInfo(default_output = compact)],
        }),
        OciLayerInfo(metadata = metadata, compact = compact, inputs = spec, platform = platform),
    ]

def _oci_layer_from_tar_impl(ctx: AnalysisContext) -> list[Provider]:
    """Retain an existing archive as the exact layer without unpacking or repacking."""
    platform = oci_platform(ctx.attrs.platform)
    metadata = ctx.actions.declare_output("layer.json")
    toolchain = ctx.attrs.toolchain[OciToolchainInfo]
    ctx.actions.run(cmd_args(
        toolchain.operations, "layer-from-tar", "--img", toolchain.img, "--platform", platform,
        "--archive", ctx.attrs.src, "--metadata", metadata.as_output(),
    ), category = "oci_layer_from_tar", allow_cache_upload = True)
    return [
        DefaultInfo(default_output = metadata, sub_targets = {"blob": [DefaultInfo(default_output = ctx.attrs.src)]}),
        OciLayerInfo(metadata = metadata, blob = ctx.attrs.src, platform = platform),
    ]

def _oci_image_impl(ctx: AnalysisContext) -> list[Provider]:
    """Compose checked small metadata without reading or copying layer payloads."""
    platform = oci_platform(ctx.attrs.platform)
    layers = [dep[OciLayerInfo] for dep in ctx.attrs.layers]
    base = ctx.attrs.base[OciImageInfo] if ctx.attrs.base != None else None
    for content in layers + ([base] if base != None else []):
        if content.platform != platform:
            fail("OCI platform mismatch: image '{}' cannot consume '{}'".format(platform, content.platform))
    if not layers and base == None:
        fail("oci_image requires layers or a base image")
    if ctx.attrs.working_dir not in [None, "", "/"]:
        _path(ctx.attrs.working_dir)
    manifest = ctx.actions.declare_output("manifest.json")
    config = ctx.actions.declare_output("config.json")
    descriptor = ctx.actions.declare_output("descriptor.json")
    spec = ctx.actions.write_json("image-inputs.json", {
        "layers": [layer.metadata for layer in layers],
        "base_manifest": base.manifest if base != None else None,
        "base_config": base.config if base != None else None,
        "base_descriptor": base.descriptor if base != None else None,
        "entrypoint": ctx.attrs.entrypoint,
        "cmd": ctx.attrs.cmd,
        "env": ctx.attrs.env,
        "labels": ctx.attrs.labels,
        "user": ctx.attrs.user,
        "working_dir": ctx.attrs.working_dir,
    }, with_inputs = True)
    toolchain = ctx.attrs.toolchain[OciToolchainInfo]
    ctx.actions.run(cmd_args(
        toolchain.operations, "image", "--img", toolchain.img, "--platform", platform, "--spec", spec,
        "--manifest", manifest.as_output(), "--config", config.as_output(),
        "--descriptor", descriptor.as_output(),
    ), category = "oci_image", allow_cache_upload = True)
    return [
        DefaultInfo(default_output = descriptor, sub_targets = {
            "manifest": [DefaultInfo(default_output = manifest)],
            "config": [DefaultInfo(default_output = config)],
        }),
        OciImageInfo(
            manifest = manifest, config = config, descriptor = descriptor, platform = platform,
            layers = (base.layers if base != None else []) + layers,
            layouts = base.layouts if base != None else [],
        ),
    ]

def _oci_import_impl(ctx: AnalysisContext) -> list[Provider]:
    """Expose checked metadata from a complete explicitly acquired base layout."""
    platform = oci_platform(ctx.attrs.platform)
    manifest = ctx.actions.declare_output("manifest.json")
    config = ctx.actions.declare_output("config.json")
    descriptor = ctx.actions.declare_output("descriptor.json")
    toolchain = ctx.attrs.toolchain[OciToolchainInfo]
    ctx.actions.run(cmd_args(
        toolchain.operations, "import", "--platform", platform, "--layout", ctx.attrs.layout,
        "--manifest", manifest.as_output(), "--config", config.as_output(),
        "--descriptor", descriptor.as_output(),
    ), category = "oci_import", allow_cache_upload = True)
    return [
        DefaultInfo(default_output = descriptor, sub_targets = {
            "manifest": [DefaultInfo(default_output = manifest)],
            "config": [DefaultInfo(default_output = config)],
        }),
        OciImageInfo(
            manifest = manifest, config = config, descriptor = descriptor, platform = platform,
            layers = [], layouts = [ctx.attrs.layout],
        ),
    ]

def _oci_layout_impl(ctx: AnalysisContext) -> list[Provider]:
    """Export and verify every referenced blob into an independent complete layout."""
    image = ctx.attrs.image[OciImageInfo]
    spec = ctx.actions.write_json("layout-inputs.json", oci_layout_spec(image), with_inputs = True)
    output = ctx.actions.declare_output("layout", dir = True)
    toolchain = ctx.attrs.toolchain[OciToolchainInfo]
    ctx.actions.run(cmd_args(
        toolchain.operations, "layout", "--img", toolchain.img, "--platform", image.platform, "--spec", spec,
        "--output", output.as_output(),
    ), category = "oci_layout", allow_cache_upload = True)
    return [DefaultInfo(default_output = output)]

_toolchain = attrs.toolchain_dep(providers = [OciToolchainInfo])

_placement_attrs = {
    "files": attrs.dict(key = attrs.string(), value = attrs.source(allow_directory = True), default = {}),
    "executables": attrs.dict(key = attrs.string(), value = attrs.source(), default = {}),
    "symlinks": attrs.dict(key = attrs.string(), value = attrs.string(), default = {}),
    "platform": attrs.string(),
    "toolchain": _toolchain,
}

oci_layer = rule(impl = _oci_layer_impl, attrs = _placement_attrs,
    doc = "Creates a compact layer from placed files, directories, Linux executables, and literal symlinks.")

oci_layer_from_tar = rule(impl = _oci_layer_from_tar_impl, attrs = {
    "src": attrs.source(),
    "platform": attrs.string(),
    "toolchain": _toolchain,
}, doc = "Uses a declared tar or gzip archive as an unchanged OCI layer payload.")

oci_image = rule(impl = _oci_image_impl, attrs = {
    "layers": attrs.list(attrs.dep(providers = [OciLayerInfo]), default = []),
    "base": attrs.option(attrs.dep(providers = [OciImageInfo]), default = None),
    "platform": attrs.string(),
    "entrypoint": attrs.option(attrs.list(attrs.string()), default = None),
    "cmd": attrs.option(attrs.list(attrs.string()), default = None),
    "env": attrs.dict(key = attrs.string(), value = attrs.string(), default = {}),
    "labels": attrs.dict(key = attrs.string(), value = attrs.string(), default = {}),
    "user": attrs.option(attrs.string(), default = None),
    "working_dir": attrs.option(attrs.string(), default = None),
    "toolchain": _toolchain,
}, doc = "Composes an OCI image from metadata; None inherits a base config field and an empty value clears it.")

oci_import = rule(impl = _oci_import_impl, attrs = {
    "layout": attrs.source(allow_directory = True),
    "platform": attrs.string(),
    "toolchain": _toolchain,
}, doc = "Validates and imports a complete OCI layout acquired outside the build action.")

oci_layout = rule(impl = _oci_layout_impl, attrs = {
    "image": attrs.dep(providers = [OciImageInfo]),
    "toolchain": _toolchain,
}, doc = "Exports a complete validated OCI layout with independent payload files.")

# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# Buck's own BUCK files still use Meta's Rust macro schema. Keep this adapter
# limited to the rules required to build Buck; new targets should use the
# public prelude directly.

load("@prelude//utils:type_defs.bzl", "is_select")
load("@bsmr_build//rules:targets.bzl", "translate_target")

prelude = native

_CFG_BUCK_BUILD = "--cfg=buck_build"

def rust_library(
    name,
    edition = None,
    rustc_flags = [],
    deps = [],
    named_deps = None,
    test_deps = None,
    test_env = None,
    autocargo = None,
    unittests = None,
    mapped_srcs = {},
    cpp_deps = None,
    cxx_bridge = None,
    visibility = ["PUBLIC"],
    **kwargs,
):
    _unused = (test_deps, test_env, named_deps, autocargo, unittests, visibility, cpp_deps, cxx_bridge)  # @unused

    prelude.rust_library(
        name = name,
        edition = edition or _default_rust_edition(),
        rustc_flags = rustc_flags + [_CFG_BUCK_BUILD],
        deps = _fix_deps(deps),
        visibility = ["PUBLIC"],
        mapped_srcs = _maybe_select_map(mapped_srcs, _fix_mapped_srcs),
        **kwargs,
    )

def rust_binary(
    name,
    edition = None,
    rustc_flags = [],
    deps = [],
    autocargo = None,
    unittests = None,
    allocator = None,
    default_strip_mode = None,
    visibility = ["PUBLIC"],
    **kwargs,
):
    _unused = (unittests, allocator, default_strip_mode, autocargo)  # @unused

    prelude.rust_binary(
        name = name,
        edition = edition or _default_rust_edition(),
        rustc_flags = rustc_flags + [_CFG_BUCK_BUILD],
        deps = _fix_deps(deps),
        visibility = visibility,
        **kwargs,
    )

def rust_unittest(name, edition = None, rustc_flags = [], deps = [], visibility = ["PUBLIC"], **kwargs):
    prelude.rust_test(
        name = name,
        edition = edition or _default_rust_edition(),
        rustc_flags = rustc_flags + [_CFG_BUCK_BUILD],
        deps = _fix_deps(deps),
        visibility = visibility,
        **kwargs,
    )

def rust_protobuf_library(
    name,
    srcs,
    build_script,
    protos = None,
    deps = None,
    test_deps = None,
    doctests = True,
    build_env = None,
    proto_srcs = None,
    crate_name = None,
):
    _unused = test_deps  # @unused
    build_name = name + "-build-prost"
    proto_name = name + "-proto-prost"

    rust_binary(
        name = build_name,
        srcs = [build_script],
        crate_root = build_script,
        deps = ["root//app/bsmr_protoc_dev:bsmr_protoc_dev"],
    )

    build_env = build_env or {}
    build_env.update({
        "PROTOC": "$(exe bsmr_build//third-party/proto:protoc)",
        "PROTOC_INCLUDE": "$(location bsmr_build//third-party/proto:google_protobuf)",
    })
    if proto_srcs:
        build_env["BUCK_PROTO_SRCS"] = "$(location {})".format(proto_srcs)

    prelude.genrule(
        name = proto_name,
        srcs = (protos or []) + ["bsmr_build//third-party/proto:google_protobuf"],
        out = ".",
        cmd = "$(exe :" + build_name + ")",
        env = build_env,
    )

    rust_library(
        name = name + "_prost",
        crate = crate_name or name,
        srcs = srcs,
        doctests = doctests,
        env = {
            "OUT_DIR": "$(location :{})".format(proto_name),
        },
        deps = [
            "bsmr_build//third-party/rust:prost",
            "bsmr_build//third-party/rust:tonic",
            "bsmr_build//third-party/rust:tonic-prost",
        ] + (deps or []),
        rustc_flags = ["-Aunused-crate-dependencies"],
    )

    native.alias(
        name = name,
        actual = ":" + name + "_prost",
        visibility = ["PUBLIC"],
    )

ProtoSrcsInfo = provider(fields = ["srcs"])

def _proto_srcs_impl(ctx):
    srcs = {src.basename: src for src in ctx.attrs.srcs}
    for dep in ctx.attrs.deps:
        for src in dep[ProtoSrcsInfo].srcs:
            if src.basename in srcs:
                fail("Duplicate src:", src.basename)
            srcs[src.basename] = src
    out = ctx.actions.copied_dir(ctx.attrs.name, srcs, has_content_based_path = False)
    return [DefaultInfo(default_output = out), ProtoSrcsInfo(srcs = srcs.values())]

proto_srcs = rule(
    impl = _proto_srcs_impl,
    attrs = {
        "deps": attrs.list(attrs.dep(), default = []),
        "srcs": attrs.list(attrs.source(), default = []),
    },
)

def _maybe_select_map(value, mapper):
    if is_select(value):
        return select_map(value, mapper)
    return mapper(value)

def _fix_mapped_srcs(srcs: dict[str, str]):
    return {translate_target(source): path for (source, path) in srcs.items()}

def _fix_deps(deps):
    if is_select(deps):
        return select_map(deps, lambda child_targets: _fix_deps(child_targets))
    return map(translate_target, deps)

def _default_rust_edition():
    package = native.package_name()
    if package:
        components = package.split("/")
        for count in range(len(components)):
            parent = "/".join(components[:len(components) - count])
            edition = read_config("rust", "default_edition:" + parent)
            if edition != None:
                return edition

    return read_config("rust", "default_edition")

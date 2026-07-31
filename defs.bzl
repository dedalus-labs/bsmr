# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

load("@bsmr_build//rules:targets.bzl", "translate_target")
load("@prelude//decls:common.bzl", "buck")
load("@prelude//os_lookup:defs.bzl", "Os", "OsLookup")

def _bsmr_bundle_impl(ctx: AnalysisContext) -> list[Provider]:
    """
    Produce a directory layout that is similar to the one our release binary
    uses, this allows setting a path for Tpx relative to BSMR_BINARY_DIR.
    """
    target_is_windows = ctx.attrs._target_os_type[OsLookup].os == Os("windows")

    binary_extension = ".exe" if target_is_windows else ""
    bsmr_binary = "bsmr" + binary_extension
    bsmr_tpx_binary = "bsmr-tpx" + binary_extension
    bsmr_daemon_binary = "bsmr-daemon" + binary_extension
    bsmr_health_check_binary = "bsmr-health-check" + binary_extension

    copied_dir = {}
    materialisations = []

    bsmr = ctx.attrs.bsmr[DefaultInfo].default_outputs[0]
    copied_dir[bsmr_daemon_binary] = bsmr
    materialisations.extend(ctx.attrs.bsmr[DefaultInfo].other_outputs)

    bsmr_client = ctx.attrs.bsmr_client[DefaultInfo].default_outputs[0]
    copied_dir[bsmr_binary] = bsmr_client
    materialisations.extend(ctx.attrs.bsmr_client[DefaultInfo].other_outputs)

    if ctx.attrs.bsmr_health_check:
        bsmr_health_check = ctx.attrs.bsmr_health_check[DefaultInfo].default_outputs[0]
        copied_dir[bsmr_health_check_binary] = bsmr_health_check
        materialisations.extend(ctx.attrs.bsmr_health_check[DefaultInfo].other_outputs)

    if ctx.attrs.tpx:
        tpx = ctx.attrs.tpx[DefaultInfo].default_outputs[0]
        copied_dir[bsmr_tpx_binary] = ctx.actions.symlink_file(bsmr_tpx_binary, tpx, has_content_based_path = False)
        materialisations.extend(ctx.attrs.tpx[DefaultInfo].other_outputs)

    out = ctx.actions.copied_dir("out", copied_dir, has_content_based_path = False)

    return [DefaultInfo(out, other_outputs = materialisations), RunInfo(cmd_args(out.project("bsmr" + binary_extension), hidden = materialisations))]

_bsmr_bundle = rule(
    impl = _bsmr_bundle_impl,
    attrs = {
        "bsmr": attrs.dep(),
        "bsmr_client": attrs.dep(),
        "bsmr_health_check": attrs.option(attrs.dep(), default = None),
        "labels": attrs.list(attrs.string(), default = []),
        "tpx": attrs.option(attrs.dep(), default = None),
        "_target_os_type": buck.target_os_type_arg(),
    },
)

def bsmr_bundle(bsmr, bsmr_client, bsmr_health_check, tpx, **kwargs):
    _bsmr_bundle(
        bsmr = translate_target(bsmr),
        bsmr_client = translate_target(bsmr_client),
        # @oss-disable[end= ]: bsmr_health_check = bsmr_health_check,
        # @oss-disable[end= ]: tpx = tpx,
        **kwargs,
    )

def _pagable_transition_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    val = refs.val[ConstraintValueInfo]
    new_cfg = ConfigurationInfo(
        constraints = platform.configuration.constraints | {val.setting.label: val},
        values = platform.configuration.values,
    )
    return PlatformInfo(
        label = platform.label,
        configuration = new_cfg,
    )

_pagable_transition = transition(
    impl = _pagable_transition_impl,
    refs = {
        "val": translate_target("root//packages/rust/starlark/starlark:pagable[enabled]"),
    },
)

def _pagable_alias_impl(ctx: AnalysisContext) -> list[Provider]:
    return ctx.attrs.actual.providers

_pagable_transition_alias = rule(
    impl = _pagable_alias_impl,
    attrs = {
        "actual": attrs.dep(),
        "labels": attrs.list(attrs.string(), default = []),
    },
    cfg = _pagable_transition,
)

def pagable_transition_alias(name: str, actual):
    _pagable_transition_alias(
        name = name,
        actual = translate_target(actual),
    )

# ===----------------------------------------------------------------------===
# Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc
# Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

load("@prelude//utils:bsmrconfig.bzl", "read_bool")
load("@prelude//utils:type_defs.bzl", "is_bool")
load(":types.bzl", "BsmrconfigBackedModifierInfo", "ConditionalModifierInfo")

def _convert_to_conditional_modifier_infos(modifiers: list[Dependency]) -> list[ConditionalModifierInfo]:
    return [modifier[ConditionalModifierInfo] for modifier in modifiers]

def _platform_deps_to_conditional_modifier_infos(platform_deps: list[Dependency]) -> list[ConditionalModifierInfo]:
    result = []
    for dep in platform_deps:
        platform_info = dep[PlatformInfo]
        for constraint_setting, constraint_value_info in platform_info.configuration.constraints.items():
            result.append(
                ConditionalModifierInfo(
                    inner = constraint_value_info,
                    key = constraint_setting,
                )
            )
    return result

def _impl(ctx: AnalysisContext) -> list[Provider]:
    pre_platform_modifiers = _platform_deps_to_conditional_modifier_infos(ctx.attrs.pre_platform_platforms) + _convert_to_conditional_modifier_infos(
        ctx.attrs.pre_platform_modifiers
    )
    post_platform_modifiers = _platform_deps_to_conditional_modifier_infos(ctx.attrs.post_platform_platforms) + _convert_to_conditional_modifier_infos(
        ctx.attrs.post_platform_modifiers
    )
    pre_cli_modifiers = _platform_deps_to_conditional_modifier_infos(ctx.attrs.pre_cli_platforms) + _convert_to_conditional_modifier_infos(
        ctx.attrs.pre_cli_modifiers
    )

    return [
        DefaultInfo(),
        BsmrconfigBackedModifierInfo(
            pre_platform_modifiers = pre_platform_modifiers, post_platform_modifiers = post_platform_modifiers, pre_cli_modifiers = pre_cli_modifiers
        ),
    ]

_bsmrconfig_backed_modifiers = rule(
    impl = _impl,
    is_configuration_rule = True,
    attrs = {
        "post_platform_modifiers": attrs.list(attrs.dep(providers = [ConditionalModifierInfo])),
        "post_platform_platforms": attrs.list(attrs.dep(providers = [PlatformInfo]), default = []),
        "pre_cli_modifiers": attrs.list(attrs.dep(providers = [ConditionalModifierInfo])),
        "pre_cli_platforms": attrs.list(attrs.dep(providers = [PlatformInfo]), default = []),
        "pre_platform_modifiers": attrs.list(attrs.dep(providers = [ConditionalModifierInfo])),
        "pre_platform_platforms": attrs.list(attrs.dep(providers = [PlatformInfo]), default = []),
    },
)

# Supports specifying one or more modifiers or platforms that can be enabled by a bsmrconfig.
BsmrconfigBackedModifier = record(
    section = str,
    property = str,
    value = str | bool | None,
    modifiers = field(list[str], default = []),
    platforms = field(list[str], default = []),
    oncall = str,
)

IS_SET_VALUE = "__is_set__"

def _bsmrconfig_matches(entry: BsmrconfigBackedModifier) -> bool:
    if is_bool(entry.value):
        return read_bool(entry.section, entry.property, required = False) == entry.value
    elif entry.value == IS_SET_VALUE:
        return read_config(entry.section, entry.property) != None
    else:
        return read_config(entry.section, entry.property) == entry.value

def _convert_to_modifiers(entries: list[BsmrconfigBackedModifier]) -> list[str]:
    modifiers = []
    for entry in entries:
        if _bsmrconfig_matches(entry):
            modifiers += entry.modifiers
    return modifiers

def _convert_to_platforms(entries: list[BsmrconfigBackedModifier]) -> list[str]:
    platforms = []
    for entry in entries:
        if _bsmrconfig_matches(entry):
            platforms += entry.platforms
    return platforms

def bsmrconfig_backed_modifiers(
    name: str, pre_platform: list[BsmrconfigBackedModifier], post_platform: list[BsmrconfigBackedModifier], pre_cli: list[BsmrconfigBackedModifier]
):
    """
    Enable bsmrconfigs to become modifiers.
    We need this so that we can migrate `read_config`s to selects without having to worry about
    having all builds specifying these bsmrconfigs to specify modifiers for them instead.

    Bsmrconfigs are read from `toolchains` cell in order to identify bsmrconfigs
    specified in modefiles on CLI, as opposed to bsmrconfigs specified in .bsmr files per cells.

    Each `BsmrconfigBackedModifier` entry can specify `modifiers` (individual constraint value targets)
    and/or `platforms` (platform targets whose constraints are applied). At least one must be non-empty.

    Args:
    `pre_platform`: These modifiers apply before the legacy target
        platform, so they can get overriden by the legacy platform + every modifier specified.
    `post_platform`: These modifiers apply after the legacy target platform, so they can override
        the legacy platform but PACKAGE/target/CLI modifiers all override them.
    `pre_cli`: These modifiers apply before the CLI modifiers, so they can override PACKAGE
        and target modifiers but still can be overriden by CLI modifiers.

    """
    for entries in (pre_platform, post_platform, pre_cli):
        for entry in entries:
            if not entry.modifiers and not entry.platforms:
                fail("BsmrconfigBackedModifier for {}.{} must specify at least one of `modifiers` or `platforms`".format(entry.section, entry.property))

    _bsmrconfig_backed_modifiers(
        name = name,
        pre_platform_modifiers = _convert_to_modifiers(pre_platform),
        post_platform_modifiers = _convert_to_modifiers(post_platform),
        pre_cli_modifiers = _convert_to_modifiers(pre_cli),
        pre_platform_platforms = _convert_to_platforms(pre_platform),
        post_platform_platforms = _convert_to_platforms(post_platform),
        pre_cli_platforms = _convert_to_platforms(pre_cli),
    )

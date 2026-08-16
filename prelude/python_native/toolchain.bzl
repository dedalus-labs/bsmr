# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Defines BSMR's finite, digest-pinned native Python toolchain catalog.

load("@prelude//:prelude.bzl", "native")
load("@prelude//toolchains:python.bzl", "python_bootstrap_toolchain")

PythonNativeDistributionInfo = provider(fields = {
    "binary": provider_field(Artifact),
    "platform": provider_field(str),
    "root": provider_field(Artifact),
    "version": provider_field(str),
})

_DEFAULT_PYTHON_VERSION = "3.14.7"
_PYTHON_RELEASE = "20260807"
_UV_VERSION = "0.12.5"
_RUFF_VERSION = "0.16.3"
_TY_VERSION = "0.0.72"

_ARTIFACT_PLATFORMS = {
    "linux-arm64": "aarch64-unknown-linux-gnu",
    "linux-x86_64": "x86_64-unknown-linux-gnu",
    "macos-arm64": "aarch64-apple-darwin",
    "macos-x86_64": "x86_64-apple-darwin",
}

_PYTHON_VERSIONS = {
    "3.13": "3.13.15",
    "3.13.15": "3.13.15",
    "3.14": "3.14.7",
    "3.14.7": "3.14.7",
}

_PYTHON_SERIES = {
    "3.13.15": "3.13",
    "3.14.7": "3.14",
}

_PYTHON_ARCHIVES = {
    "3.13.15": {
        "linux-arm64": ("1dfc9565c26f8892a33202b5966bdf9ff45c56a57b06e8fa65fecf05030afe5b", 29253383),
        "linux-x86_64": ("faae10a9faa9bec06da009ac69326cc1d9691dc138fec6a1b69159dff1781f35", 34822151),
        "macos-arm64": ("dbadb0ffe46f8bace50daaf8a0c5fc6903c003690776da9eb5269e33c856bb53", 25156281),
        "macos-x86_64": ("187eed2282e9c3a5b6b14953d564ee25a9f35cf2c209c9fa292186ee48b0e4a1", 24927967),
    },
    "3.14.7": {
        "linux-arm64": ("4dba8d7e06199f841a9d6b54e4eb58d446a5c20c65085a916190dd0162c6e93b", 30243151),
        "linux-x86_64": ("a2478d654ed51d443bae21ec20ad927f116b4f5aae4094ab74918a6aa38f0575", 35940499),
        "macos-arm64": ("532645be202d3df8510ab318ca5faed92060a694c8327144e14fa81f31af0f6d", 26499186),
        "macos-x86_64": ("9964e2d618ebea03be8ea3e65ab0ecc0f2b030ce203345b8f92654641fd4de66", 26601297),
    },
}

_ARCHIVES = {
    "ruff": {
        "linux-arm64": ("b9cc833f5db856484b38718c9da195a6ec990707307bda30530913a09705419a", 10126329),
        "linux-x86_64": ("7ab3b978d2c0b1c96b2323d4e5c4f35284ae1cdf35d2f7399595c74c805f5fa3", 10649747),
        "macos-arm64": ("136a4db6512d9b16dda56ac8604696ed65c3b1a914a142de029e7f8d5006f1d9", 9944235),
        "macos-x86_64": ("05c2a6705e7c0c056d6d93ff538978583f0c47b4c28d334ab9d58d2e8daf4c24", 10742637),
    },
    "ty": {
        "linux-arm64": ("80e7d05a2620fc3a57335888d4f46f67fef348999eee7e0c515eb8c115009f94", 11614321),
        "linux-x86_64": ("11087394fa6aeac8d449bce42a30eb03fd76dd9ad8f38e69dd8004785d3f98b0", 12350680),
        "macos-arm64": ("061ff070c830bd82c960c55352e0f4bce05d9093b44efda16ae2feeeea0032a5", 11574363),
        "macos-x86_64": ("88e1e344d8f86f05f7041a233b58d93a6ff7ab080232611a5de3ea521f18d833", 11891418),
    },
    "uv": {
        "linux-arm64": ("9bf43b4d1a07665bf64d4c4e710930b382321a785e0eb10aac07f46471f86a31", 21478307),
        "linux-x86_64": ("68a509da24b06b4223a1c0175fb5eb5bc79342b76cbeff0cfe51ac3f5b17b6b2", 23015306),
        "macos-arm64": ("5bb0e5fe008a773c3dbcb97ff79cd89e1241464fe9d2f986d52ad8f1b037bd62", 18518284),
        "macos-x86_64": ("b3b2137477cf96c9686ebfb71524614cec780c673fd73e59bce099aef02e70e8", 20848713),
    },
}

def _distribution_impl(ctx: AnalysisContext) -> list[Provider]:
    """Exposes one executable while retaining its complete distribution tree."""
    binary = ctx.attrs.root.project(ctx.attrs.executable)
    return [
        DefaultInfo(default_output = binary),
        PythonNativeDistributionInfo(binary = binary, platform = ctx.attrs.platform, root = ctx.attrs.root, version = ctx.attrs.version),
        RunInfo(args = [binary]),
    ]

_distribution = rule(
    impl = _distribution_impl,
    attrs = {
        "executable": attrs.string(),
        "platform": attrs.string(),
        "root": attrs.source(allow_directory = True),
        "version": attrs.string(),
    },
)

def python_native_platform_value(values: dict):
    """Selects one catalog value for the execution OS and CPU."""
    return select({
        "config//os:linux": select({
            "config//cpu:arm64": values["linux-arm64"],
            "config//cpu:x86_64": values["linux-x86_64"],
        }),
        "config//os:macos": select({
            "config//cpu:arm64": values["macos-arm64"],
            "config//cpu:x86_64": values["macos-x86_64"],
        }),
    })

def _python_version() -> str:
    """Returns one supported exact Python version from root configuration."""
    requested = native.read_root_config("python", "version", _DEFAULT_PYTHON_VERSION)
    version = _PYTHON_VERSIONS.get(requested)
    if version == None:
        fail("unsupported Python version '{}'; supported versions are {}".format(requested, sorted(_PYTHON_VERSIONS)))
    return version

def python_native_python_platform_value(values: dict):
    """Selects one value for the configured Python line, execution OS, and CPU."""
    series = _PYTHON_SERIES[_python_version()]
    return python_native_platform_value({
        platform: values["{}-{}".format(series, platform)]
        for platform in _ARTIFACT_PLATFORMS
    })

def _archive(name: str, tool: str, version: str, url: str, strip_prefix: str, archives = None) -> None:
    """Declares one digest-verified platform distribution."""
    archives = archives if archives != None else _ARCHIVES[tool]
    native.http_archive(
        name = name,
        has_content_based_path = True,
        sha256 = python_native_platform_value({platform: value[0] for platform, value in archives.items()}),
        size_bytes = python_native_platform_value({platform: value[1] for platform, value in archives.items()}),
        strip_prefix = python_native_platform_value({platform: strip_prefix.format(platform = _ARTIFACT_PLATFORMS[platform]) for platform in archives}),
        urls = [python_native_platform_value({platform: url.format(platform = _ARTIFACT_PLATFORMS[platform], version = version) for platform in archives})],
    )

def python_native_toolchain() -> None:
    """Declares independently addressable latest-stable Python tools."""
    python_version = _python_version()
    _archive(
        "__bsmr_python_archive",
        "python",
        python_version,
        "https://github.com/astral-sh/python-build-standalone/releases/download/{release}/cpython-{version}%2B{release}-{platform}-install_only_stripped.tar.gz".format(release = _PYTHON_RELEASE, version = "{version}", platform = "{platform}"),
        "python",
        archives = _PYTHON_ARCHIVES[python_version],
    )
    for tool, version in [("uv", _UV_VERSION), ("ruff", _RUFF_VERSION), ("ty", _TY_VERSION)]:
        _archive(
            "__bsmr_{}_archive".format(tool),
            tool,
            version,
            "https://github.com/astral-sh/{tool}/releases/download/{{version}}/{tool}-{{platform}}.tar.gz".format(tool = tool),
            "{}-{{platform}}".format(tool),
        )
    platform = python_native_platform_value(_ARTIFACT_PLATFORMS)
    _distribution(name = "__bsmr_python_distribution", executable = "bin/python3", platform = platform, root = ":__bsmr_python_archive", version = python_version, visibility = ["PUBLIC"])
    python_bootstrap_toolchain(name = "python_bootstrap", interpreter = ":__bsmr_python_distribution", visibility = ["PUBLIC"])
    _distribution(name = "__bsmr_uv_distribution", executable = "uv", platform = platform, root = ":__bsmr_uv_archive", version = _UV_VERSION, visibility = ["PUBLIC"])
    _distribution(name = "__bsmr_ruff_distribution", executable = "ruff", platform = platform, root = ":__bsmr_ruff_archive", version = _RUFF_VERSION, visibility = ["PUBLIC"])
    _distribution(name = "__bsmr_ty_distribution", executable = "ty", platform = platform, root = ":__bsmr_ty_archive", version = _TY_VERSION, visibility = ["PUBLIC"])

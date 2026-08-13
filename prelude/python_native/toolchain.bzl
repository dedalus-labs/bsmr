# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Defines BSMR's finite, digest-pinned native Python toolchain catalog.

load("@prelude//:prelude.bzl", "native")

PythonNativeDistributionInfo = provider(fields = {
    "binary": provider_field(Artifact),
    "root": provider_field(Artifact),
    "version": provider_field(str),
})

_PYTHON_VERSION = "3.14.7"
_PYTHON_RELEASE = "20260807"
_UV_VERSION = "0.12.3"
_RUFF_VERSION = "0.16.2"
_TY_VERSION = "0.0.70"

_ARTIFACT_PLATFORMS = {
    "linux-arm64": "aarch64-unknown-linux-gnu",
    "linux-x86_64": "x86_64-unknown-linux-gnu",
    "macos-arm64": "aarch64-apple-darwin",
    "macos-x86_64": "x86_64-apple-darwin",
}

_ARCHIVES = {
    "python": {
        "linux-arm64": ("4dba8d7e06199f841a9d6b54e4eb58d446a5c20c65085a916190dd0162c6e93b", 30243151),
        "linux-x86_64": ("a2478d654ed51d443bae21ec20ad927f116b4f5aae4094ab74918a6aa38f0575", 35940499),
        "macos-arm64": ("532645be202d3df8510ab318ca5faed92060a694c8327144e14fa81f31af0f6d", 26499186),
        "macos-x86_64": ("9964e2d618ebea03be8ea3e65ab0ecc0f2b030ce203345b8f92654641fd4de66", 26601297),
    },
    "ruff": {
        "linux-arm64": ("b2a2a2573455cc33af98f8a8fb49294c02d4e2e4a7f9e81844411f0a57f30318", 10434638),
        "linux-x86_64": ("3d2c355e641ceb5b608a158c603768fcc908c5009c56c6e78da7487da033b92a", 11078307),
        "macos-arm64": ("fbf6cbc23d254b0bc03a6fb2b1b04efb917fe5ce068d027e735ce7ed65b9bed6", 10048909),
        "macos-x86_64": ("6648fa7a7c95b087c5b9d269d8b9a567fae091bdef3993f77cc7531a01bd7266", 10678791),
    },
    "ty": {
        "linux-arm64": ("5996a7bdd7eb93548030ce084006cc722d3fb984dd38e5b403f4e2e99ae87d38", 11697395),
        "linux-x86_64": ("6e44d58998d7b16b630d6229f1002a6b2ed28e56cc856d16c996ed257e1e7fde", 12456363),
        "macos-arm64": ("50076094d3ebbf98749ac395b9fb6fcc25cb9ba84a419ca9d5956e221b37302a", 11133591),
        "macos-x86_64": ("4ebfac284659a7050b24e97ba0575a2d66dbb62190015f1275c78706a4d089f3", 11811011),
    },
    "uv": {
        "linux-arm64": ("bb66cb52e7b1823aed1183630d8d8e5c958840d584a4c55ec10a4cfc168dcca2", 20423730),
        "linux-x86_64": ("600cf9a742aca00d292673b16b5acffaa7b8c269a364ad0c2e79498dcb1fe101", 21721441),
        "macos-arm64": ("546f7f8a6c70ff13a3a9d2bc958db3427298cebf3e0cb756f9177133b7068843", 17686637),
        "macos-x86_64": ("4c9f52262a14da336e4a42ed24992d12d0c956acde87619e4611d321dffa602b", 19547702),
    },
}

def _distribution_impl(ctx: AnalysisContext) -> list[Provider]:
    """Exposes one executable while retaining its complete distribution tree."""
    binary = ctx.attrs.root.project(ctx.attrs.executable)
    return [
        DefaultInfo(default_output = binary),
        PythonNativeDistributionInfo(binary = binary, root = ctx.attrs.root, version = ctx.attrs.version),
    ]

_distribution = rule(
    impl = _distribution_impl,
    attrs = {
        "executable": attrs.string(),
        "root": attrs.source(allow_directory = True),
        "version": attrs.string(),
    },
)

def _platform_value(values: dict):
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

def _archive(name: str, tool: str, version: str, url: str, strip_prefix: str) -> None:
    """Declares one digest-verified platform distribution."""
    native.http_archive(
        name = name,
        has_content_based_path = True,
        sha256 = _platform_value({platform: value[0] for platform, value in _ARCHIVES[tool].items()}),
        size_bytes = _platform_value({platform: value[1] for platform, value in _ARCHIVES[tool].items()}),
        strip_prefix = _platform_value({platform: strip_prefix.format(platform = _ARTIFACT_PLATFORMS[platform]) for platform in _ARCHIVES[tool]}),
        urls = [_platform_value({platform: url.format(platform = _ARTIFACT_PLATFORMS[platform], version = version) for platform in _ARCHIVES[tool]})],
    )

def python_native_toolchain() -> None:
    """Declares independently addressable latest-stable Python tools."""
    _archive(
        "__bsmr_python_archive",
        "python",
        _PYTHON_VERSION,
        "https://github.com/astral-sh/python-build-standalone/releases/download/{release}/cpython-{version}%2B{release}-{platform}-install_only_stripped.tar.gz".format(release = _PYTHON_RELEASE, version = "{version}", platform = "{platform}"),
        "python",
    )
    for tool, version in [("uv", _UV_VERSION), ("ruff", _RUFF_VERSION), ("ty", _TY_VERSION)]:
        _archive(
            "__bsmr_{}_archive".format(tool),
            tool,
            version,
            "https://github.com/astral-sh/{tool}/releases/download/{{version}}/{tool}-{{platform}}.tar.gz".format(tool = tool),
            "{}-{{platform}}".format(tool),
        )
    _distribution(name = "__bsmr_python_distribution", executable = "bin/python3", root = ":__bsmr_python_archive", version = _PYTHON_VERSION, visibility = ["PUBLIC"])
    _distribution(name = "__bsmr_uv_distribution", executable = "uv", root = ":__bsmr_uv_archive", version = _UV_VERSION, visibility = ["PUBLIC"])
    _distribution(name = "__bsmr_ruff_distribution", executable = "ruff", root = ":__bsmr_ruff_archive", version = _RUFF_VERSION, visibility = ["PUBLIC"])
    _distribution(name = "__bsmr_ty_distribution", executable = "ty", root = ":__bsmr_ty_archive", version = _TY_VERSION, visibility = ["PUBLIC"])

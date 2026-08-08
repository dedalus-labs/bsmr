# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Defines exact pnpm toolchains and a frozen repository-install action.

"""RFC 0001's milestone-zero pnpm adapter.

This is RFC 0001's milestone-zero adapter. pnpm remains responsible for
interpreting its manifest and lockfile, while BSMR owns the tool distributions,
declared project inputs, mutable state, action key, and cached install output.
"""

NodeDistributionInfo = provider(fields = {
    "node": provider_field(RunInfo),
    "version": provider_field(str),
})

PnpmDistributionInfo = provider(fields = {
    "cli": provider_field(Artifact),
    "package_manager": provider_field(str),
    "version": provider_field(str),
})

PnpmToolchainInfo = provider(fields = {
    "node": provider_field(RunInfo),
    "node_version": provider_field(str),
    "package_manager": provider_field(str),
    "pnpm_cli": provider_field(Artifact),
})

PnpmInstallInfo = provider(fields = {
    "node_modules": provider_field(Artifact),
    "workspace": provider_field(Artifact),
})

def _require_exact_version(name: str, version: str) -> None:
    """Require an unadorned three-component semantic version."""
    parts = version.split(".")
    if len(parts) != 3:
        fail("{} version '{}' must be an exact semantic version".format(name, version))
    for part in parts:
        if part == "" or str(int(part)) != part:
            fail("{} version '{}' must be an exact semantic version".format(name, version))

def _parse_package_manager(package_manager: str) -> str:
    """Validate a Corepack-style pnpm pin and return its exact version."""
    prefix = "pnpm@"
    separator = "+sha512."
    if not package_manager.startswith(prefix):
        fail("package_manager must start with '{}'".format(prefix))
    components = package_manager[len(prefix):].split(separator)
    if len(components) != 2:
        fail("package_manager must contain one '{}' digest separator".format(separator))
    version, digest = components
    _require_exact_version("pnpm", version)
    major = int(version.split(".")[0])
    if major != 10 and major != 11:
        fail("pnpm toolchains support exact pnpm 10 and 11 versions")
    if len(digest) != 128:
        fail("package_manager must contain a lowercase 128-character sha512 digest")
    for character in digest.elems():
        if character not in "0123456789abcdef":
            fail("package_manager must contain a lowercase 128-character sha512 digest")
    return version

def _node_distribution_impl(ctx: AnalysisContext) -> list[Provider]:
    """Expose an exact Node executable from a verified distribution tree."""
    _require_exact_version("Node", ctx.attrs.version)
    node = ctx.attrs.root.project(ctx.attrs.executable)
    return [
        DefaultInfo(default_output = node),
        NodeDistributionInfo(
            node = RunInfo(args = [node]),
            version = ctx.attrs.version,
        ),
    ]

node_distribution = rule(
    impl = _node_distribution_impl,
    attrs = {
        "executable": attrs.string(default = "bin/node", doc = "Node executable path relative to the distribution root."),
        "root": attrs.source(allow_directory = True, doc = "Node distribution extracted from a digest-verified archive."),
        "version": attrs.string(doc = "Exact Node version exposed by the distribution."),
    },
    doc = "Exposes an exact Node runtime from a downloaded distribution.",
)

def _pnpm_distribution_impl(ctx: AnalysisContext) -> list[Provider]:
    """Expose the pnpm CLI named by an exact package-manager pin."""
    version = _parse_package_manager(ctx.attrs.package_manager)
    cli = ctx.attrs.root.project(ctx.attrs.cli)
    return [
        DefaultInfo(default_output = cli),
        PnpmDistributionInfo(
            cli = cli,
            package_manager = ctx.attrs.package_manager,
            version = version,
        ),
    ]

pnpm_distribution = rule(
    impl = _pnpm_distribution_impl,
    attrs = {
        "cli": attrs.string(default = "package/bin/pnpm.cjs", doc = "pnpm CLI path relative to the extracted npm package."),
        "package_manager": attrs.string(doc = "Exact pnpm@version+sha512.digest value from package.json."),
        "root": attrs.source(allow_directory = True, doc = "pnpm package extracted from a digest-verified archive."),
    },
    doc = "Exposes an exact pnpm CLI from a downloaded npm package.",
)

def _pnpm_toolchain_impl(ctx: AnalysisContext) -> list[Provider]:
    """Combine exact Node and pnpm distributions into one execution toolchain."""
    node = ctx.attrs.node[NodeDistributionInfo]
    pnpm = ctx.attrs.pnpm[PnpmDistributionInfo]
    return [
        DefaultInfo(),
        PnpmToolchainInfo(
            node = node.node,
            node_version = node.version,
            package_manager = pnpm.package_manager,
            pnpm_cli = pnpm.cli,
        ),
    ]

pnpm_toolchain = rule(
    impl = _pnpm_toolchain_impl,
    attrs = {
        "node": attrs.exec_dep(providers = [NodeDistributionInfo]),
        "pnpm": attrs.exec_dep(providers = [PnpmDistributionInfo]),
    },
    is_toolchain_rule = True,
    doc = "Defines the exact Node and pnpm executables used by pnpm actions.",
)

def _validate_project_path(path: str) -> None:
    """Reject absolute, parent-traversing, and build-system-owned input paths."""
    components = path.replace("\\", "/").split("/")
    if path == "" or path.startswith("/") or (len(path) > 1 and path[1] == ":"):
        fail("pnpm project input path '{}' must be relative".format(path))
    if "" in components or "." in components or ".." in components:
        fail("pnpm project input path '{}' must be normalized".format(path))
    if components[0] == ".bsmr":
        fail("pnpm project inputs may not use BSMR's reserved '.bsmr' path")

def _pnpm_install_impl(ctx: AnalysisContext) -> list[Provider]:
    """Run one frozen pnpm install over the complete declared project tree."""
    if type(ctx.attrs.srcs) == type([]):
        sources = {source.short_path: source for source in ctx.attrs.srcs}
        if len(sources) != len(ctx.attrs.srcs):
            fail("pnpm project inputs contain duplicate short paths; use an explicit path-to-source mapping")
    else:
        sources = dict(ctx.attrs.srcs)
    for path in sources:
        _validate_project_path(path)
    for path, source in {
        "package.json": ctx.attrs.package_json,
        "pnpm-lock.yaml": ctx.attrs.pnpm_lock,
    }.items():
        if path in sources:
            fail("pnpm project input path '{}' is reserved by the rule".format(path))
        sources[path] = source

    source_tree = ctx.actions.symlinked_dir(
        "__{}_pnpm_srcs__".format(ctx.label.name),
        sources,
        has_content_based_path = False,
    )
    workspace = ctx.actions.declare_output(ctx.label.name, dir = True, has_content_based_path = False)
    toolchain = ctx.attrs._pnpm_toolchain[PnpmToolchainInfo]
    command = cmd_args([
        toolchain.node,
        ctx.attrs._runner,
        "--source",
        source_tree,
        "--output",
        workspace.as_output(),
        "--pnpm-cli",
        toolchain.pnpm_cli,
        "--package-manager",
        toolchain.package_manager,
        "--node-version",
        toolchain.node_version,
    ])
    ctx.actions.run(
        command,
        category = "pnpm_install",
        identifier = ctx.label.name,
        local_only = True,
    )

    node_modules = workspace.project("node_modules", hide_prefix = True)
    return [
        DefaultInfo(default_output = node_modules, other_outputs = [workspace]),
        PnpmInstallInfo(node_modules = node_modules, workspace = workspace),
    ]

pnpm_install = rule(
    impl = _pnpm_install_impl,
    attrs = {
        "package_json": attrs.source(doc = "Root package.json with exact engines.node and packageManager fields."),
        "pnpm_lock": attrs.source(doc = "Authoritative pnpm-lock.yaml consumed with --frozen-lockfile."),
        "srcs": attrs.one_of(
            attrs.list(attrs.source()),
            attrs.dict(attrs.string(), attrs.source()),
            default = [],
            doc = "All remaining project sources, optionally mapped to explicit project-relative paths.",
        ),
        "_pnpm_toolchain": attrs.default_only(attrs.toolchain_dep(default = "toolchains//:pnpm", providers = [PnpmToolchainInfo])),
        "_runner": attrs.source(default = "prelude//toolchains/pnpm:runner"),
    },
    doc = "Materializes one cached pnpm workspace from a frozen lockfile.",
)

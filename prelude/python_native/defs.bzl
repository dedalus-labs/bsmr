# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Defines cache-separable Python environment, build, lint, and typecheck actions.

load(":toolchain.bzl", "PythonNativeDistributionInfo")

PythonEnvironmentInfo = provider(fields = {
    "root": provider_field(Artifact),
})

PythonSourcesInfo = provider(fields = {
    "files": provider_field(dict[str, Artifact]),
    "tree": provider_field(Artifact),
})

PythonVcsInfo = provider(fields = {
    "tree": provider_field(Artifact),
})

PythonWheelInfo = provider(fields = {
    "directory": provider_field(Artifact),
})

def _validate_project_path(path: str) -> None:
    """Rejects non-normalized and build-system-owned Python input paths."""
    components = path.replace("\\", "/").split("/")
    if path == "" or path.startswith("/") or (len(path) > 1 and path[1] == ":"):
        fail("Python project input path '{}' must be relative".format(path))
    if "" in components or "." in components or ".." in components:
        fail("Python project input path '{}' must be normalized".format(path))
    if components[0] in [".bsmr", ".git", ".venv", "__pycache__", "build", "dist"]:
        fail("Python project input path '{}' is generated or owned by BSMR".format(path))

def _python_sources_impl(ctx: AnalysisContext) -> list[Provider]:
    """Materializes one declared first-party source tree."""
    for path in ctx.attrs.srcs:
        _validate_project_path(path)
    tree = ctx.actions.symlinked_dir(ctx.label.name, ctx.attrs.srcs, has_content_based_path = False)
    return [
        DefaultInfo(default_output = tree),
        PythonSourcesInfo(files = ctx.attrs.srcs, tree = tree),
    ]

python_sources = rule(
    impl = _python_sources_impl,
    attrs = {"srcs": attrs.dict(attrs.string(), attrs.source())},
    doc = "Defines one Python project's source-controlled files.",
)

def _python_vcs_impl(ctx: AnalysisContext) -> list[Provider]:
    """Materializes the minimal declared Git database required by build backends."""
    tree = ctx.actions.symlinked_dir(ctx.label.name, ctx.attrs.srcs, has_content_based_path = True)
    return [DefaultInfo(default_output = tree), PythonVcsInfo(tree = tree)]

python_vcs = rule(
    impl = _python_vcs_impl,
    attrs = {"srcs": attrs.dict(attrs.string(), attrs.source(allow_directory = True))},
    doc = "Defines version-control state as an explicit build input.",
)

def _tool_arg(dependency: Dependency):
    """Returns one executable while retaining its complete distribution tree."""
    tool = dependency[PythonNativeDistributionInfo]
    return cmd_args(tool.binary, hidden = [tool.root]), tool.version

def _runner_command(ctx: AnalysisContext, mode: str, output: Artifact):
    """Constructs one action command from only the tools consumed by its mode."""
    python, python_version = _tool_arg(ctx.attrs.python)
    command = cmd_args(
        [
            python,
            ctx.attrs._runner,
            mode,
            "--output",
            output.as_output(),
            "--python",
            python,
        ],
    )
    if mode in ["environment", "wheel", "wheel-environment"]:
        tool, version = _tool_arg(ctx.attrs.uv)
        command.add(["--uv", tool])
    elif mode == "ruff":
        tool, version = _tool_arg(ctx.attrs.ruff)
        command.add(["--ruff", tool])
    elif mode == "ty":
        tool, version = _tool_arg(ctx.attrs.ty)
        command.add(["--ty", tool])
    else:
        fail("unknown native Python action mode '{}'".format(mode))
    return command, python_version, version

def _add_config_settings(command, settings: dict) -> None:
    """Adds typed PEP 517 settings without exposing shell interpretation."""
    for name, values in settings.items():
        for value in values:
            command.add(["--config-setting={}={}".format(name, value)])

def _python_environment_impl(ctx: AnalysisContext) -> list[Provider]:
    """Materializes the exact PEP 751 installation set with pinned uv."""
    root = ctx.actions.declare_output(ctx.label.name, dir = True, has_content_based_path = True)
    command, python_version, uv_version = _runner_command(ctx, "environment", root)
    command.add(["--lock", ctx.attrs.lock])
    _add_config_settings(command, ctx.attrs.config_settings)
    if ctx.attrs.build_environment != None:
        command.add([
            "--build-environment",
            ctx.attrs.build_environment[PythonEnvironmentInfo].root,
        ])
    ctx.actions.run(
        command,
        allow_cache_upload = True,
        category = "python_environment",
        identifier = "python-{}-uv-{}".format(python_version, uv_version),
        local_only = True,
    )
    return [
        DefaultInfo(default_output = root),
        PythonEnvironmentInfo(root = root),
    ]

python_environment = rule(
    impl = _python_environment_impl,
    attrs = {
        "build_environment": attrs.option(
            attrs.dep(providers = [PythonEnvironmentInfo]),
            default = None,
        ),
        "config_settings": attrs.dict(attrs.string(), attrs.list(attrs.string()), default = {}),
        "lock": attrs.source(doc = "Canonical PEP 751 installation set."),
        "python": attrs.exec_dep(providers = [PythonNativeDistributionInfo]),
        "uv": attrs.exec_dep(providers = [PythonNativeDistributionInfo]),
        "_runner": attrs.source(default = "prelude//python_native:runner"),
    },
    doc = "Materializes one cached environment from a PEP 751 lock and explicit build closure.",
)

def _python_wheel_environment_impl(ctx: AnalysisContext) -> list[Provider]:
    """Materializes exact first-party wheels independently of locked dependencies."""
    root = ctx.actions.declare_output(ctx.label.name, dir = True, has_content_based_path = True)
    command, python_version, uv_version = _runner_command(ctx, "wheel-environment", root)
    for wheel in ctx.attrs.wheels:
        command.add(["--wheel-dir", wheel[PythonWheelInfo].directory])
    ctx.actions.run(
        command,
        allow_cache_upload = True,
        category = "python_wheel_environment",
        identifier = "python-{}-uv-{}".format(python_version, uv_version),
        local_only = True,
    )
    return [
        DefaultInfo(default_output = root),
        PythonEnvironmentInfo(root = root),
    ]

python_wheel_environment = rule(
    impl = _python_wheel_environment_impl,
    attrs = {
        "python": attrs.exec_dep(providers = [PythonNativeDistributionInfo]),
        "uv": attrs.exec_dep(providers = [PythonNativeDistributionInfo]),
        "wheels": attrs.list(attrs.dep(providers = [PythonWheelInfo])),
        "_runner": attrs.source(default = "prelude//python_native:runner"),
    },
    doc = "Materializes first-party wheel metadata as an independently cached runtime layer.",
)

def _project_action(ctx: AnalysisContext, mode: str, output: Artifact) -> None:
    """Registers one first-party action over an exact source and dependency tree."""
    if ctx.attrs.project_root != ".":
        _validate_project_path(ctx.attrs.project_root)
    sources = ctx.attrs.sources[PythonSourcesInfo]
    command, _, version = _runner_command(ctx, mode, output)
    command.add([
        "--project-root",
        ctx.attrs.project_root,
        "--source",
        sources.tree,
    ])
    if mode in ["ty", "wheel"]:
        environment = ctx.attrs.environment[PythonEnvironmentInfo]
        command.add(["--environment", environment.root])
    if mode == "wheel" and ctx.attrs.vcs != None:
        command.add(["--vcs", ctx.attrs.vcs[PythonVcsInfo].tree])
    if mode == "wheel":
        _add_config_settings(command, ctx.attrs.config_settings)
    ctx.actions.run(
        command,
        allow_cache_upload = True,
        category = "python_{}".format(mode),
        identifier = version,
        local_only = mode == "wheel",
    )

def _python_wheel_impl(ctx: AnalysisContext) -> list[Provider]:
    """Builds one first-party wheel through its declared PEP 517 backend."""
    output = ctx.actions.declare_output(ctx.label.name, dir = True, has_content_based_path = True)
    _project_action(ctx, "wheel", output)
    return [DefaultInfo(default_output = output), PythonWheelInfo(directory = output)]

def _check_impl(mode: str):
    """Returns one check rule implementation for the selected Astral tool."""
    def implementation(ctx: AnalysisContext) -> list[Provider]:
        output = ctx.actions.declare_output("{}.check".format(ctx.label.name))
        _project_action(ctx, mode, output)
        return [DefaultInfo(default_output = output)]
    return implementation

def _project_attrs(tool: str, needs_environment: bool = False) -> dict:
    """Returns the narrow schema for one first-party Python action."""
    attrs_by_name = {
        "python": attrs.exec_dep(providers = [PythonNativeDistributionInfo]),
        "project_root": attrs.string(),
        "sources": attrs.dep(providers = [PythonSourcesInfo]),
        tool: attrs.exec_dep(providers = [PythonNativeDistributionInfo]),
        "_runner": attrs.source(default = "prelude//python_native:runner"),
    }
    if needs_environment:
        attrs_by_name["environment"] = attrs.dep(providers = [PythonEnvironmentInfo])
    if tool == "uv":
        attrs_by_name["config_settings"] = attrs.dict(attrs.string(), attrs.list(attrs.string()), default = {})
        attrs_by_name["vcs"] = attrs.option(attrs.dep(providers = [PythonVcsInfo]), default = None)
    return attrs_by_name

python_wheel = rule(
    impl = _python_wheel_impl,
    attrs = _project_attrs("uv", needs_environment = True),
    doc = "Builds a reproducible PEP 517 wheel.",
)
ruff_check = rule(
    impl = _check_impl("ruff"),
    attrs = _project_attrs("ruff"),
    doc = "Checks one project with pinned Ruff.",
)
ty_check = rule(
    impl = _check_impl("ty"),
    attrs = _project_attrs("ty", needs_environment = True),
    doc = "Checks one project with pinned ty.",
)

def _runtime_command(ctx: AnalysisContext, mode: str) -> cmd_args:
    """Constructs one runtime command from exact source, environment, and interpreter inputs."""
    python, _ = _tool_arg(ctx.attrs.python)
    command = cmd_args([
        python,
        ctx.attrs._runtime,
    ])
    for environment in ctx.attrs.environments:
        command.add(["--environment", environment[PythonEnvironmentInfo].root])
    command.add([
        "--project-root",
        ctx.attrs.project_root,
        "--source",
        ctx.attrs.sources[PythonSourcesInfo].tree,
    ])
    if mode == "entry":
        command.add(["--entry", ctx.attrs.entry])
    command.add(mode)
    return command

def _runtime_attrs(entry: bool = False) -> dict:
    """Returns the closed schema for Python tests and entry points."""
    result = {
        "environments": attrs.list(attrs.dep(providers = [PythonEnvironmentInfo])),
        "project_root": attrs.string(),
        "python": attrs.exec_dep(providers = [PythonNativeDistributionInfo]),
        "sources": attrs.dep(providers = [PythonSourcesInfo]),
        "_runtime": attrs.source(default = "prelude//python_native:runtime"),
    }
    if entry:
        result["entry"] = attrs.string()
    return result

def _python_entry_point_impl(ctx: AnalysisContext) -> list[Provider]:
    """Exposes one PEP 621 console script through ``bsmr run``."""
    command = _runtime_command(ctx, "entry")
    return [DefaultInfo(), RunInfo(args = command)]

python_entry_point = rule(
    impl = _python_entry_point_impl,
    attrs = _runtime_attrs(entry = True),
    doc = "Runs one standard Python console-script entry point.",
)

def _python_test_impl(ctx: AnalysisContext) -> list[Provider]:
    """Exposes one pytest suite through BSMR's test protocol."""
    test = ExternalRunnerTestInfo(
        type = "python",
        command = [_runtime_command(ctx, "test")],
        contacts = [],
        labels = [],
        run_from_project_root = True,
    )
    return [test, RunInfo(args = test.command), DefaultInfo()]

python_test = rule(
    impl = _python_test_impl,
    attrs = _runtime_attrs(),
    doc = "Runs pytest from a named PEP 751 test environment.",
)

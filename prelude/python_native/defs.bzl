# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Defines cache-separable Python environment, build, lint, and typecheck actions.

load(":toolchain.bzl", "PythonNativeDistributionInfo")

PythonEnvironmentInfo = provider(fields = {
    "identity": provider_field(Artifact),
    "roots": provider_field(list[Artifact]),
})

PythonLockedArtifactInfo = provider(fields = {
    "file": provider_field(Artifact),
})

PythonLockedPackageInfo = provider(fields = {
    "manifest": provider_field(Artifact),
    "name": provider_field(str),
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
    tree = ctx.actions.symlinked_dir(
        ctx.label.name,
        ctx.attrs.srcs,
        has_content_based_path = False,
    )
    return [
        DefaultInfo(default_output = tree),
        PythonSourcesInfo(files = ctx.attrs.srcs, tree = tree),
    ]

python_sources = rule(
    impl = _python_sources_impl,
    attrs = {
        "srcs": attrs.dict(attrs.string(), attrs.source()),
    },
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

def _runner_command(ctx: AnalysisContext, mode: str, output):
    """Constructs one action command from only the tools consumed by its mode."""
    python_toolchain = ctx.attrs.python[PythonNativeDistributionInfo]
    python, python_version = _tool_arg(ctx.attrs.python)
    command = cmd_args(
        [
            python,
            ctx.attrs._runner,
            mode,
            "--output",
            output,
            "--python",
            python,
            "--python-platform",
            python_toolchain.platform,
        ],
    )
    if mode in ["locked-package", "select-package", "wheel", "wheel-environment"]:
        tool, version = _tool_arg(ctx.attrs.uv)
        command.add(["--uv", tool])
    elif mode in ["compose-environment", "validate-environments"]:
        version = python_version
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

def _locked_package_command(ctx: AnalysisContext, root, manifest, lock: Artifact, source: bool, absent: bool, artifacts, requirement):
    """Constructs one wheel or source installation with exact cache semantics."""
    command, python_version, uv_version = _runner_command(ctx, "locked-package", root)
    command.add(["--lock", lock, "--manifest", manifest])
    if absent:
        command.add("--absent")
    if artifacts != None:
        if ctx.attrs.artifact != None:
            fail("direct and selected wheel artifacts are mutually exclusive")
        if source:
            fail("a source package cannot consume selected wheel artifacts")
        for artifact in artifacts:
            command.add(["--artifact", artifact[PythonLockedArtifactInfo].file])
        command.add(["--requirement", requirement])
    if ctx.attrs.artifact != None:
        if source:
            fail("a source package cannot consume a wheel artifact")
        command.add(["--artifact", ctx.attrs.artifact[PythonLockedArtifactInfo].file])
    if source:
        if ctx.attrs.build_environment == None:
            fail("source package '{}' requires a declared build environment".format(ctx.attrs.package))
        if ctx.attrs.source_artifact != None:
            command.add(["--source-artifact", ctx.attrs.source_artifact[PythonLockedArtifactInfo].file])
        elif ctx.attrs.source_tree != None:
            command.add(["--source-tree", ctx.attrs.source_tree])
        if ctx.attrs.source_artifact != None or ctx.attrs.source_tree != None:
            command.add(["--distribution", ctx.attrs.package, "--version", ctx.attrs.source_version])
            if ctx.attrs.source_subdirectory != None:
                command.add(["--source-subdirectory", ctx.attrs.source_subdirectory])
        _add_config_settings(command, ctx.attrs.config_settings)
        for setting in ctx.attrs.package_config_settings:
            command.add(["--package-config-setting", setting])
        for variable in ctx.attrs.package_build_variables:
            command.add(["--package-build-variable", variable])
        environment = ctx.attrs.build_environment[PythonEnvironmentInfo]
        for build_root in environment.roots:
            command.add(["--build-environment", build_root])
        command.add(cmd_args(hidden = [environment.identity]))
    return command, python_version, uv_version

def _register_locked_package(ctx: AnalysisContext, actions, root, manifest, lock: Artifact, source: bool, absent: bool, artifacts, requirement) -> None:
    """Registers one package action after artifact selection is final."""
    command, python_version, uv_version = _locked_package_command(
        ctx,
        root,
        manifest,
        lock,
        source,
        absent,
        artifacts,
        requirement,
    )
    actions.run(
        command,
        # PEP 517 builds still consume the local execution-platform compiler.
        allow_cache_upload = not source,
        category = "python_locked_package",
        identifier = "python-{}-uv-{}".format(python_version, uv_version),
        local_only = source,
    )

def _selected_locked_package(ctx: AnalysisContext, artifacts, outputs, selection: Artifact, root: Artifact, manifest: Artifact, lock: Artifact) -> None:
    """Binds a mixed package to uv's exact wheel-or-source decision."""
    selected = json.decode(artifacts[selection].read_string())
    if type(selected) != "dict":
        fail("uv emitted a non-object package selection")
    acquisition = selected.get("acquisition")
    if acquisition == "wheel":
        if sorted(selected.keys()) != ["acquisition", "requirement"] or type(selected["requirement"]) != "string":
            fail("uv emitted invalid wheel selection {}".format(selected))
        source = False
        selected_artifacts = ctx.attrs.artifacts
        requirement = selected["requirement"] if selected_artifacts != None else None
        absent = False
    elif acquisition == "source":
        if sorted(selected.keys()) != ["acquisition"] or ctx.attrs.acquisition != "wheel-or-source":
            fail("uv emitted invalid source selection {}".format(selected))
        source = True
        selected_artifacts = None
        requirement = None
        absent = False
    elif acquisition == "absent":
        if sorted(selected.keys()) != ["acquisition"]:
            fail("uv emitted invalid absent selection {}".format(selected))
        source = False
        selected_artifacts = None
        requirement = None
        absent = True
    else:
        fail("uv emitted invalid package selection {}".format(selected))
    _register_locked_package(
        ctx,
        ctx.actions,
        outputs[root].as_output(),
        outputs[manifest].as_output(),
        lock,
        source,
        absent,
        selected_artifacts,
        requirement,
    )

def _python_locked_package_impl(ctx: AnalysisContext) -> list[Provider]:
    """Materializes one normalized package from a canonical PEP 751 fragment."""
    if ctx.attrs.artifact != None and ctx.attrs.acquisition != "wheel":
        fail("a direct wheel artifact requires wheel acquisition")
    if ctx.attrs.artifact != None and ctx.attrs.artifacts != None:
        fail("direct and selected wheel artifacts are mutually exclusive")
    if ctx.attrs.source_artifact != None and ctx.attrs.acquisition == "wheel":
        fail("a source artifact requires source acquisition")
    if ctx.attrs.source_tree != None and ctx.attrs.acquisition == "wheel":
        fail("a source tree requires source acquisition")
    if ctx.attrs.source_artifact != None and ctx.attrs.source_tree != None:
        fail("source artifacts and source trees are mutually exclusive")
    source_input = ctx.attrs.source_artifact != None or ctx.attrs.source_tree != None
    if source_input and ctx.attrs.source_version == None:
        fail("a source input requires its locked version")
    if not source_input and (ctx.attrs.source_subdirectory != None or ctx.attrs.source_version != None):
        fail("source metadata requires a source input")
    root = ctx.actions.declare_output(ctx.label.name, dir = True, has_content_based_path = True)
    manifest = ctx.actions.declare_output("{}.manifest.json".format(ctx.label.name), has_content_based_path = True)
    lock = ctx.actions.write("pylock.{}.toml".format(ctx.label.name), ctx.attrs.lock)
    if ctx.attrs.acquisition == "wheel-or-source" or ctx.attrs.artifacts != None:
        selection = ctx.actions.declare_output("{}.selection".format(ctx.label.name))
        command, python_version, uv_version = _runner_command(
            ctx,
            "select-package",
            selection.as_output(),
        )
        command.add(["--lock", lock, "--distribution", ctx.attrs.package])
        if ctx.attrs.acquisition == "wheel-or-source":
            command.add("--source-permitted")
        ctx.actions.run(
            command,
            allow_cache_upload = True,
            category = "python_package_selection",
            identifier = "python-{}-uv-{}".format(python_version, uv_version),
        )
        ctx.actions.dynamic_output(
            dynamic = [selection],
            inputs = [],
            outputs = [root.as_output(), manifest.as_output()],
            f = lambda dynamic_ctx, artifacts, outputs: _selected_locked_package(
                dynamic_ctx,
                artifacts,
                outputs,
                selection,
                root,
                manifest,
                lock,
            ),
        )
    else:
        _register_locked_package(
            ctx,
            ctx.actions,
            root.as_output(),
            manifest.as_output(),
            lock,
            ctx.attrs.acquisition == "source",
            False,
            None,
            None,
        )
    return [
        DefaultInfo(default_output = root, other_outputs = [manifest]),
        PythonLockedPackageInfo(
            manifest = manifest,
            name = ctx.attrs.package,
            root = root,
        ),
    ]

python_locked_package = rule(
    impl = _python_locked_package_impl,
    attrs = {
        "acquisition": attrs.enum(["source", "wheel", "wheel-or-source"]),
        "artifact": attrs.option(
            attrs.dep(providers = [PythonLockedArtifactInfo]),
            default = None,
        ),
        "artifacts": attrs.option(
            attrs.list(attrs.dep(providers = [PythonLockedArtifactInfo])),
            default = None,
        ),
        "build_environment": attrs.option(
            attrs.dep(providers = [PythonEnvironmentInfo]),
            default = None,
        ),
        "config_settings": attrs.dict(attrs.string(), attrs.list(attrs.string()), default = {}),
        "lock": attrs.string(doc = "Canonical one-package PEP 751 installation set."),
        "package": attrs.string(doc = "Normalized Python distribution name."),
        "package_build_variables": attrs.list(attrs.string(), default = []),
        "package_config_settings": attrs.list(attrs.string(), default = []),
        "python": attrs.exec_dep(providers = [PythonNativeDistributionInfo]),
        "source_artifact": attrs.option(
            attrs.dep(providers = [PythonLockedArtifactInfo]),
            default = None,
        ),
        "source_subdirectory": attrs.option(attrs.string(), default = None),
        "source_tree": attrs.option(attrs.source(allow_directory = True), default = None),
        "source_version": attrs.option(attrs.string(), default = None),
        "uv": attrs.exec_dep(providers = [PythonNativeDistributionInfo]),
        "_runner": attrs.source(default = "prelude//python_native:runner"),
    },
    doc = "Materializes one package-granular CAS tree with pinned uv.",
)

def _python_locked_artifact_impl(ctx: AnalysisContext) -> list[Provider]:
    """Acquires one immutable wheel as a first-class action input."""
    wheel = ctx.actions.declare_output(
        ctx.attrs.filename,
        has_content_based_path = True,
    )
    ctx.actions.download_file(
        wheel.as_output(),
        ctx.attrs.url,
        sha256 = ctx.attrs.sha256,
        size_bytes = ctx.attrs.size,
        has_content_based_path = True,
    )
    return [
        DefaultInfo(default_output = wheel),
        PythonLockedArtifactInfo(file = wheel),
    ]

python_locked_artifact = rule(
    impl = _python_locked_artifact_impl,
    attrs = {
        "filename": attrs.string(),
        "sha256": attrs.string(),
        "size": attrs.int(),
        "url": attrs.string(),
    },
    doc = "Downloads one digest-verified locked distribution artifact.",
)

def _python_environment_impl(ctx: AnalysisContext) -> list[Provider]:
    """Composes package CAS artifacts into one deterministic import root."""
    overlay = ctx.actions.declare_output(
        "{}.overlay".format(ctx.label.name),
        dir = True,
        has_content_based_path = True,
    )
    manifest = ctx.actions.declare_output(
        "{}.manifest.json".format(ctx.label.name),
        has_content_based_path = True,
    )
    command, python_version, _ = _runner_command(
        ctx,
        "compose-environment",
        overlay.as_output(),
    )
    command.add(["--manifest", manifest.as_output()])
    for package in ctx.attrs.packages:
        package = package[PythonLockedPackageInfo]
        command.add(["--package", package.name, package.manifest, package.root])
    ctx.actions.run(
        command,
        allow_cache_upload = True,
        category = "python_environment",
        identifier = "python-{}".format(python_version),
    )
    return [
        DefaultInfo(default_output = manifest, other_outputs = [overlay]),
        PythonEnvironmentInfo(identity = manifest, roots = [overlay]),
    ]

python_environment = rule(
    impl = _python_environment_impl,
    attrs = {
        "packages": attrs.list(attrs.dep(providers = [PythonLockedPackageInfo])),
        "python": attrs.exec_dep(providers = [PythonNativeDistributionInfo]),
        "_runner": attrs.source(default = "prelude//python_native:runner"),
    },
    doc = "Composes package-granular Python trees with recorded precedence.",
)

def _python_wheel_environment_impl(ctx: AnalysisContext) -> list[Provider]:
    """Materializes exact first-party wheels independently of locked dependencies."""
    root = ctx.actions.declare_output(ctx.label.name, dir = True, has_content_based_path = True)
    command, python_version, uv_version = _runner_command(ctx, "wheel-environment", root.as_output())
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
        PythonEnvironmentInfo(identity = root, roots = [root]),
    ]

python_wheel_environment = rule(
    impl = _python_wheel_environment_impl,
    attrs = {
        "python": attrs.exec_dep(providers = [PythonNativeDistributionInfo]),
        "uv": attrs.exec_dep(providers = [PythonNativeDistributionInfo]),
        "wheels": attrs.list(attrs.dep(providers = [PythonWheelInfo])),
        "_runner": attrs.source(default = "prelude//python_native:runner"),
    },
    doc = "Materializes first-party wheels as an independently cached runtime layer.",
)

def _project_action(ctx: AnalysisContext, mode: str, output: Artifact) -> None:
    """Registers one first-party action over an exact source and dependency tree."""
    if ctx.attrs.project_root != ".":
        _validate_project_path(ctx.attrs.project_root)
    sources = ctx.attrs.sources[PythonSourcesInfo]
    command, _, version = _runner_command(ctx, mode, output.as_output())
    command.add([
        "--project-root",
        ctx.attrs.project_root,
        "--source",
        sources.tree,
    ])
    if mode == "wheel":
        environment = ctx.attrs.environment[PythonEnvironmentInfo]
        for root in environment.roots:
            command.add(["--environment", root])
        command.add(cmd_args(hidden = [environment.identity]))
    elif mode == "ty":
        for environment in ctx.attrs.environments:
            environment = environment[PythonEnvironmentInfo]
            for root in environment.roots:
                command.add(["--environment", root])
            command.add(cmd_args(hidden = [environment.identity]))
    if mode in ["wheel", "ty"]:
        environments = [ctx.attrs.environment] if mode == "wheel" else ctx.attrs.environments
        command.add(cmd_args(hidden = [_validate_environment_stack(ctx, environments)]))
    if mode == "wheel" and ctx.attrs.vcs != None:
        command.add(["--vcs", ctx.attrs.vcs[PythonVcsInfo].tree])
    if mode == "wheel":
        _add_config_settings(command, ctx.attrs.config_settings)
        for setting in ctx.attrs.package_config_settings:
            command.add(["--package-config-setting", setting])
        for variable in ctx.attrs.package_build_variables:
            command.add(["--package-build-variable", variable])
    ctx.actions.run(
        command,
        # PEP 517 outputs are not remotely reusable until the native toolchain is declared.
        allow_cache_upload = mode != "wheel",
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
        if tool == "ty":
            attrs_by_name["environments"] = attrs.list(attrs.dep(providers = [PythonEnvironmentInfo]))
        else:
            attrs_by_name["environment"] = attrs.dep(providers = [PythonEnvironmentInfo])
    if tool == "uv":
        attrs_by_name["config_settings"] = attrs.dict(attrs.string(), attrs.list(attrs.string()), default = {})
        attrs_by_name["package_build_variables"] = attrs.list(attrs.string(), default = [])
        attrs_by_name["package_config_settings"] = attrs.list(attrs.string(), default = [])
        attrs_by_name["vcs"] = attrs.option(attrs.dep(providers = [PythonVcsInfo]), default = None)
    return attrs_by_name

python_wheel = rule(
    impl = _python_wheel_impl,
    attrs = _project_attrs("uv", needs_environment = True),
    doc = "Builds a reproducible PEP 517 wheel.",
)

def _validate_environment_stack(ctx: AnalysisContext, environments: list[Dependency]) -> Artifact:
    """Registers one cacheable PEP 794 validation across import-search layers."""
    output = ctx.actions.declare_output("__bsmr_python_environment_stack.json")
    command, python_version, _ = _runner_command(
        ctx,
        "validate-environments",
        output.as_output(),
    )
    for dependency in environments:
        environment = dependency[PythonEnvironmentInfo]
        for root in environment.roots:
            command.add(["--environment", root])
        command.add(cmd_args(hidden = [environment.identity]))
    ctx.actions.run(
        command,
        allow_cache_upload = True,
        category = "python_environment_validation",
        identifier = "python-{}".format(python_version),
    )
    return output
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
    command.add(cmd_args(hidden = [_validate_environment_stack(ctx, ctx.attrs.environments)]))
    for environment in ctx.attrs.environments:
        environment = environment[PythonEnvironmentInfo]
        for root in environment.roots:
            command.add(["--environment", root])
        command.add(cmd_args(hidden = [environment.identity]))
    command.add([
        "--project-root",
        ctx.attrs.project_root,
        "--source",
        ctx.attrs.sources[PythonSourcesInfo].tree,
    ])
    if mode == "entry":
        command.add(["--entry", ctx.attrs.entry])
    elif mode == "test":
        for argument in ctx.attrs.test_command:
            command.add(["--test-command={}".format(argument)])
    command.add(mode)
    return command

def _runtime_attrs(entry: bool = False, test: bool = False) -> dict:
    """Returns the closed schema for Python tests and entry points."""
    result = {
        "environments": attrs.list(attrs.dep(providers = [PythonEnvironmentInfo])),
        "project_root": attrs.string(),
        "python": attrs.exec_dep(providers = [PythonNativeDistributionInfo]),
        "sources": attrs.dep(providers = [PythonSourcesInfo]),
        "_runner": attrs.source(default = "prelude//python_native:runner"),
        "_runtime": attrs.source(default = "prelude//python_native:runtime"),
    }
    if entry:
        result["entry"] = attrs.string()
    if test:
        result["test_command"] = attrs.list(
            attrs.string(),
            default = ["-m", "pytest"],
        )
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
    attrs = _runtime_attrs(test = True),
    doc = "Runs one declared Python test command from a named PEP 751 environment.",
)

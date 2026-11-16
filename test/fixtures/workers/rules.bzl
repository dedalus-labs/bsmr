def _workers_impl(ctx: AnalysisContext) -> list[Provider]:
    """Register local execution with persistent workers enabled."""
    platform = ExecutionPlatformInfo(
        label = ctx.label.raw_target(),
        configuration = ConfigurationInfo(constraints = {}, values = {}),
        executor_config = CommandExecutorConfig(
            local_enabled = True,
            remote_enabled = False,
            use_persistent_workers = True,
        ),
    )
    return [DefaultInfo(), ExecutionPlatformRegistrationInfo(platforms = [platform])]

workers = rule(impl = _workers_impl, attrs = {})

def _worker_impl(ctx: AnalysisContext) -> list[Provider]:
    """Run the worker script with the test interpreter and generated protocol modules."""
    python = read_config("test", "python")
    return [
        DefaultInfo(),
        WorkerInfo(
            exe = cmd_args(python, ctx.attrs.src),
            env = {"PYTHONPATH": read_config("test", "pythonpath")},
        ),
    ]

worker = rule(impl = _worker_impl, attrs = {"src": attrs.source()})

def _write_impl(ctx: AnalysisContext) -> list[Provider]:
    """Write one output through the persistent worker only."""
    output = ctx.actions.declare_output(ctx.label.name)
    ctx.actions.run(
        cmd_args(output.as_output()),
        category = "write",
        exe = WorkerRunInfo(worker = ctx.attrs.worker[WorkerInfo], exe = cmd_args("false")),
        local_only = True,
    )
    return [DefaultInfo(default_output = output)]

write = rule(impl = _write_impl, attrs = {"worker": attrs.exec_dep(providers = [WorkerInfo])})

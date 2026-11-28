def _copy_impl(ctx: AnalysisContext) -> list[Provider]:
    """Copy declared bytes with a real local command eligible for cache publication."""
    output = ctx.actions.declare_output("copy.txt")
    ctx.actions.run(
        ["/bin/cp", ctx.attrs.src, output.as_output()],
        category = "copy",
        local_only = True,
        allow_cache_upload = True,
    )
    return [DefaultInfo(default_output = output)]

copy = rule(impl = _copy_impl, attrs = {"src": attrs.source()})

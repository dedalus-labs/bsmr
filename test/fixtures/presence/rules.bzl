def _tree_impl(ctx: AnalysisContext) -> list[Provider]:
    """Write one directory artifact with a cacheable local command."""
    output = ctx.actions.declare_output("tree", dir = True)
    ctx.actions.run(
        ["sh", ctx.attrs.script, output.as_output()],
        category = "tree",
        allow_local_cache_upload = True,
    )
    return [DefaultInfo(default_output = output)]

tree = rule(impl = _tree_impl, attrs = {"script": attrs.source()})

def _use_impl(ctx: AnalysisContext) -> list[Provider]:
    """Read the whole directory artifact as one input of a separate action."""
    output = ctx.actions.declare_output(ctx.label.name + ".txt")
    tree = ctx.attrs.tree[DefaultInfo].default_outputs[0]
    ctx.actions.run(
        ["sh", "-c", 'ls "$1/d1" > "$2"', "use", tree, output.as_output()],
        category = "use",
    )
    return [DefaultInfo(default_output = output)]

use = rule(impl = _use_impl, attrs = {"tree": attrs.dep()})

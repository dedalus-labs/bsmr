<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Explains how custom actions compose with inferred package builds. -->

# Custom recipes

Use a recipe when your build needs another step, such as running a compiled
program to generate a report. The step can consume an existing package's output
without repeating how that package is built.

Suppose `app` is an executable built with the [native Rust preview](languages/rust/cargo.md).
Create `recipe/BUILD.bsmr`:

```python
genrule(
    name = "report",
    out = "report.txt",
    cmd = "$(exe root//app:app) > $OUT",
)
```

Then build the report:

```console
bsmr build recipe:report --show-output
```

`root//app:app` identifies the executable in the `app` package. `$(exe ...)`
tracks its executable and runtime inputs. `$OUT` is the output path BSMR assigns
to `report.txt`. BSMR builds the application before running the report step.

Another action can declare `recipe:report` as an input. Each step exposes
outputs for the next step, so BSMR can schedule and cache them independently.
Reusable Starlark functions can compose the same declarations.

Declare every tool, input, output, and environment value that affects the step.
A build that reads undeclared state cannot safely reuse cached results. See
[caching](../concepts/caching.md) for the complete input contract.

An explicit `BUILD.bsmr` defines its whole package and takes precedence over
native inference. Put recipes in a separate directory when the application
should keep using its language files as the build definition.

Use `bsmr build --help` for build options. Run `bsmr docs --help` to find API
documentation commands, or open the [command-line reference](../reference/cli.md)
for queries and inspection commands.

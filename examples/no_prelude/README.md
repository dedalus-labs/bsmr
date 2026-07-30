## No-prelude example

This is an example project that does not rely on
https://github.com/facebook/buck2-prelude. Instead the prelude cell points to a
`prelude` directory with an empty `prelude.bzl` file, like so:

```
#.buckconfig
[cells]
root = .
prelude = prelude
```

All rules and toolchains are defined manually within each of the subdirectories.
(e.g. `cpp/rules.bzl`, `cpp/toolchain.bzl`)

## Sample commands

Install Bessemer, cd into a project, and run

```bash
# List all targets
bsmr targets //...
# Build all targets
bsmr build //...
# Run C++ hello_world main
bsmr run //cpp/hello_world:main
```

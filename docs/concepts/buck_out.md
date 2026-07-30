---
id: buck_out
title: buck-out
---

# buck-out

Bessemer stores build artifacts in a directory named `buck-out` in the root of your
[project](glossary.md#project). You should not make assumptions about where
Bessemer places your build artifacts within the directory structure beneath
`buck-out` as these locations depend on Bessemer's implementation and could
potentially change over time. Instead, to obtain the location of the build
artifact for a particular target, you can use one of the `--show-*-output`
options with the [`bsmr build`](../../users/commands/build) or
[`bsmr targets`](../../users/commands/targets) commands, most commonly
`--show-output`. For the full list of ways to show the output location, you can
run `bsmr build --help` or `bsmr targets --help`.

```sh
bsmr targets --show-output <target>
bsmr build --show-output <target>
```

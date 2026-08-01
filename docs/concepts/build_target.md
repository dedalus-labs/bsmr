---
id: build_target
title: Build Target
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->


# Build Target

A _build target_ is a string that identifies a build target in your project.
Build targets are used as arguments to Bessemer commands, such as
[`bsmr build`](../../users/commands/build) and
[`bsmr run`](../../users/commands/run). Build targets are also used as
arguments to [build rules](build_rule.md) to enable one target to reference
another. For example, a build rule might use a build target to reference another
target in order to specify that target as a _dependency_.

#### Fully-qualified build targets

Here is an example of a _fully-qualified_ build target:

```
cell//java/com/facebook/share:ui
```

A fully-qualified build target has three components:

1. The `cell//` prefix indicates that the subsequent path is from the _root_ of
   `cell`.
2. The `java/com/facebook/share` between the `//` prefix and the colon (`:`)
   indicates that the [build file](build_file.md) (usually named `BUCK`) is
   located in the directory `java/com/facebook/share`.
3. The `ui` after the colon (`:`) indicates the name of the build target within
   the build file. Build target names must be unique within a build file. By
   _name_ we mean, more formally, the value of the `name` argument to the build
   rule.

Note that the name of the build file itself—usually BUCK—does _not_ occur in the
build target. All build files within a given Bessemer project must have the same
name—defined in the `[buildfile].name` entry of `.bsmrconfig`. Therefore, it is
unnecessary to include the name in the target. The full regular expression for a
fully-qualified build target is as follows:

```
[A-Za-z0-9._-]*//[A-Za-z0-9/._-]*:[A-Za-z0-9_/.=,@~+-]+
|- cell name -|  | package path | |--- target name ----|
```

In Bessemer, a _cell_ defines a directory tree of one or more Bessemer packages. For
more information about Bessemer cells and their relationship to packages and
projects, see the [Key Concepts](key_concepts.md) topic. **NOTE:** All target
paths are assumed to start from the root of the Bessemer project. Bessemer does not
support specifying a target path that starts from a directory below the root.
Although the double forward slash (`//`) that prefixes target paths can be
omitted when specifying a target from the command line (see **Pro Tips** below),
Bessemer still assumes that the path is from the root. Bessemer does support
_relative_ build paths, but in Bessemer, that concept refers to specifying build
targets _from within_ a build file. See **Relative build targets** below for
more details.

#### Cell relative build targets

A _cell relative_ build target omits the cell, and is inferred to be relative to
the current cell.

#### Package relative build targets

A _package relative_ build target can be used to reference a build target
_within the same _[_build file_](build_file.md) (aka _package_). A relative
build target starts with a colon (`:`) and is followed by only the third
component (or _short name_) of the fully-qualified build target. The following
snippet from a build file shows an example of using a relative path.

```python
## Assume this target is in //java/com/facebook/share/BUCK#
java_binary(
  name = 'ui_jar',
  deps = [
    ## The following target path
    ##   //java/com/facebook/share:ui
    ## is the same as using the following relative path.#
    ':ui',
  ],
)
```

## Command-line Pro Tips

Here are some ways that you can reduce your typing when you specify build
targets as command-line arguments to the `bsmr build` or `bsmr run` commands.
Consider the following example of a fully-qualified build target used with the
`bsmr build` command:

```sh
bsmr build cell//java/com/facebook/share:share
```

Although Bessemer is always strict when parsing build targets in build files, Bessemer
is flexible when parsing build targets on the command-line. Specifically, the
leading `//` is optional on the command line, so the above could be:

```sh
bsmr build java/com/facebook/share:share
```

Also, if there is a forward slash before the colon, it is ignored, so this could
also be written as:

```sh
bsmr build java/com/facebook/share/:share
```

which enables you to produce the red text shown below using tab-completion,
which dramatically reduces how much you need to type:

```sh
bsmr build java/com/facebook/share/:share
```

Finally, if the final path element matches the value specified after the colon,
it can be omitted:

```sh
# This is treated as //java/com/facebook/share:share.
bsmr build java/com/facebook/share/
```

which makes the build target even easier to tab-complete. For this reason, the
name of the build target for the primary deliverable in a build file is often
named the same as the parent directory. That way, it can be built from the
command-line with less typing.

## See also

Bessemer supports the ability to define **_aliases_ for build targets**; using
aliases can improve brevity when specifying targets on the Bessemer command line.
For more information, see the [`[alias]`](bsmrconfig.md#alias) section in the
documentation for [`.bsmrconfig`](bsmrconfig.md). A
[**build target pattern**](target_pattern.md) is a string that describes a set
of one or more build targets. For example, the pattern `//...` is used to build
an entire project. For more information, see the **Build Target Pattern** topic.

<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

## A simple Hello World project using the bsmr-prelude

This example demonstrates how a simple C++ project might be built with Bessemer
using the prelude.

In the `toolchains` cell, we define two toolchains needed:
`system_cxx_toolchain` and `system_python_bootstrap_toolchain`, both pulled in
from the prelude. The `BUILD.bsmr` file at the project root contain a `cxx_binary`
target and its `cxx_library` dependency. `.bsmr` contains the
configuration to set the target platform for the project:

```
[parser]
target_platform_detector_spec = target:root//...->prelude//platforms:default \
  target:prelude//...->prelude//platforms:default \
  target:toolchains//...->prelude//platforms:default
```

## Setup

Run `bsmr init --git`.

## Sample commands

To view all targets in the project,

```bash
bsmr targets //...
```

To build the main C++ binary,

```bash
bsmr build //:main
```

To run the main C++ binary,

```bash
# Should print "Hello from C++!"
bsmr run //:main
```

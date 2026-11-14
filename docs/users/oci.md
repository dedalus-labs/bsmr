<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Builds and runs the complete hello-world image example. -->

# Build and run a hello-world image

Build a program that prints `hello`, put it in a container image, and run it.
BSMR builds the program and image archive. Docker loads the archive and starts
the container.

## Get the example

The complete project is in `examples/oci/hello`. It includes the program,
image recipe, and pinned tools. This walkthrough uses an Apple Silicon Mac,
a BSMR build with OCI rules, Python 3, and Docker Desktop running Linux containers.
It produces a Linux arm64 image.

```sh
git clone https://github.com/dedalus-labs/bsmr.git
cd bsmr/examples/oci/hello
```

Here is the program in `cmd/hello/main.go`:

```go
package main

import "fmt"

func main() {
    fmt.Println("hello")
}
```

## Build the image

Acquire the pinned Go SDK and let BSMR discover the program:

```sh
bsmr go toolchain --version 1.26.7
bsmr go sync
```

Keep cached build results outside the checkout, then build the image archive:

```sh
export BSMR_LOCAL_CACHE_DIR="$HOME/.cache/bsmr"
bsmr build //image:hello --target-platforms //image:linux --out hello.tar
```

`//image:hello` names the `hello` target in `image/BUILD.bsmr`. Its recipe builds
the program for Linux, places it at `/hello` inside the image, and selects it
as the startup command. The target exports `hello.tar` for Docker to load.

## Run it

```sh
docker --context desktop-linux load --input hello.tar
docker --context desktop-linux run --rm --pull=never bsmr-hello:local
```

The container prints:

```text
hello
```

Change the greeting in `cmd/hello/main.go`, then repeat the build, load, and run
commands. BSMR rebuilds the affected compiler work and image layer. An
image-configuration edit, such as changing the entrypoint, can reuse the
compiled program and packed layer.

## Use your own application

Replace the program target in `image/BUILD.bsmr`. Include the files and shared
libraries your application needs, then test it in a Linux runtime. The
[rule reference](https://github.com/dedalus-labs/bsmr/blob/main/prelude/oci/README.md)
covers platforms, base images, configuration inheritance, and tool setup on
other hosts. The rule API is experimental. Pin your project's engine version.

## Compact layers

Native file and directory layers retain compact streams and their original
inputs. Export reconstructs standard OCI layer bytes. Imported archives retain
their ordinary blobs. Complete exports still require the full image bytes.

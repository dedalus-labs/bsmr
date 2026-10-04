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

## Install Debian packages

Use an engine built from this revision so its bundled rules include native
filesystem actions. The complete `examples/oci/debian` project downloads a locked Debian base and
verified package files, installs `curl` and `ca-certificates` in a private image
root, and checks the result. Docker and BuildKit are not used. This example
requires a trusted rootful Linux arm64 worker with network access for acquisition,
trusted TLS certificates, and the ordinary BSMR shell/tar bootstrap tools.
Its image commands run with networking disabled.

BSMR downloads the locked package files over HTTPS before installation. Inside
the image, `apt-get` installs those local files and runs their installation
scripts. An online `apt-get update && apt-get install` command is not supported
inside `oci_run`.

From the BSMR checkout on that worker:

```sh
cd examples/oci/debian
export BSMR_LOCAL_CACHE_DIR="$HOME/.cache/bsmr"
bsmr build //:check --show-output
bsmr build //:layout --show-output
```

The check runs the installed `curl`, checks the certificate bundle created by
package installation, and queries the installed package database. The second
command exports the image as a complete OCI directory. The project includes
the base and package locks and all tool pins.

`oci_run` executes commands against a private writable copy of the base image.
It mounts declared inputs read-only below `/inputs`, preserves the original
image's startup configuration, and rejects failed commands. Use `oci_image`
to change the entrypoint or other startup configuration afterward. Linux arm64
execution and local cache restoration are qualified. Rootless execution,
cross-host reproducibility, and remote cache upload for filesystem commands
are not qualified.

## Compact layers

Native file and directory layers retain a compact stream and their original
inputs. They do not retain a second full tar blob during metadata-only builds.
Export reconstructs and verifies standard OCI layer bytes. Imported archives
and filesystem-command results retain their ordinary blobs. This optimization
does not squash layers or remove the storage required by a complete export.

See the [rule reference](https://github.com/dedalus-labs/bsmr/blob/main/prelude/oci/README.md)
for the acquisition, execution, publication, and cache contracts.

<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines the native executor's ownership and cache boundary. -->

# Native execution

The executor sends declared files to the installed macOS worker. The worker owns
the private filesystem and process identity. The build daemon keeps the existing
command graph, action cache and output validation.

```text
prepared command -> input archive -> authenticated worker -> cleanup receipt
                                                            |
materializer <- existing output validator <- result archive -+
```

Enable the worker explicitly:

```ini
[sandbox]
backend = native
launcher_socket = /private/var/db/bsmr/control.sock
```

Use `--sandbox` with the build or test command. The socket's adjacent `.json`
record must be root-owned and name the worker's actual runtime identity. That
identity and the native profile enter the action key before cache lookup.
Missing or incompatible metadata fails the command.

The executor reserves worker capacity before creating archives. It reuses the
canonical command decoder, declared-input writer and untrusted-output validator.
The worker opens no caller-supplied host path. The client transfers already-open
file descriptors and authenticates the server's root credentials first.

Cancellation closes the request's write half, then waits for a bounded cleanup
acknowledgement. A failed or missing acknowledgement never permits output import.
Successful cleanup is separate from a successful compiler exit. The result
archive preserves the compiler's exit status and standard streams.

| Owner | Responsibility |
| --- | --- |
| `NativeExecutor` | Runtime identity, admission capacity and worker connection |
| `NativeAction` | Open input descriptors and validated output declarations |
| `bsmr_sandbox::native` | Shared protocol and authenticated transport |
| `tools/native` | Runtime files, reserved identity, execution and cleanup |

This wiring requires an installed worker with a suitable runtime seed. Installer
integration and full macOS consumer qualification remain separate release gates.

Build checks:

```console
cargo check -p bsmr_execute_impl -p bsmr_server
cargo test -p bsmr_sandbox --lib
```

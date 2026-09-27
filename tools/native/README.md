<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines the privileged native worker's process-ownership boundary. -->

# Native worker

A build can leave a child running after its main process exits. Each native
macOS action therefore needs an unused UID/GID and an exclusive kernel lock.
The supervisor stops every process with that identity before accepting outputs.
Compilation runs without root privileges.

```text
lease -> unprivileged action -> empty UID -> validate outputs -> release
```

`Identity::acquire` requires administrator-created leases and unused identities.
Action requests cannot choose either. `Identity::drain` stops detached children
and checks kernel membership. Callers must check its result before publishing.
Acquisition refuses a crashed worker's occupied UID until an administrator recovers it.

This crate currently provides process ownership. It does not yet install a
service, construct runtime images, or enable a macOS BSMR execution backend.

```console
cargo +1.98.0 test --locked --manifest-path tools/native/Cargo.toml
cargo +1.98.0 clippy --locked --manifest-path tools/native/Cargo.toml -- -D warnings
```

The identity-scoped signal follows [Nix's Darwin process cleanup](https://github.com/NixOS/nix/blob/209d2bc4428841d4446a3c3b6f75bb5bbfa0f71a/src/libutil/unix/processes.cc).
BSMR additionally requires the kernel's process set to be empty.

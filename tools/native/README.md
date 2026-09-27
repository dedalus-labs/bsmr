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

`channel::Files` passes three open files over a Unix socket: the action description,
input archive and result archive. The receiver checks the kernel's peer UID against
administrator configuration before reading the packet. It rejects devices and
incorrect access modes. Reception runs before the service starts threads or children,
so received descriptors become close-on-exec before any child can inherit them.
The worker must still validate file contents before execution.

`Archive::capture` copies input bytes into a private file with a 64 KiB buffer and
checks their SHA-256 identity. Sender writes and descriptor-offset changes cannot
alter that captured copy. `unpack` accepts bounded regular files, directories and
internal links in a new private directory. It installs links last and removes
write and privilege bits. The supervisor owns the destination and its cleanup.

`Request::read` admits the existing `GuestAction` protocol only after matching the
installed runtime identity, bounding the message and deadline, and rejecting invalid
paths, overlapping outputs and malformed commands. Admission grants no host-file
authority. Filesystem paths are interpreted only inside the prepared root.

This crate provides process ownership and the confined launch transition. It does not yet install a
service, construct runtime images, or enable a macOS BSMR execution backend.

`launch::enter` runs in a dedicated trusted child. It changes the filesystem root,
resets the working directory, drops supplementary groups and root credentials,
then denies network and host-service access through Seatbelt. The supervisor
owns the prepared root and UID lease. No action code runs until every step succeeds.
Any error terminates that child. This function must not run in a `pre_exec` hook.

`Job` binds the checked request, captured inputs, private root and identity lease.
Its owned execution task retains those resources if its waiter disappears. Only
`Completed` exposes output paths and streams, after the child and every process
with the reserved UID have stopped. The trusted launcher inherits the same lease
until credential drop, then closes it at payload exec. This prevents a surviving
root launcher from entering an identity that has already been reassigned.

`run` owns the direct child until exit, cancellation, or timeout and checks
descendant cleanup before returning. It uses Tokio's process and socket notifications, including when
the caller disconnects. The caller keeps its identity lease through output
validation. Kernel failures and cleanup failures never authorize publication.

```console
cargo +1.98.0 test --locked --manifest-path tools/native/Cargo.toml
cargo +1.98.0 clippy --locked --manifest-path tools/native/Cargo.toml -- -D warnings
```

The identity-scoped signal follows [Nix's Darwin process cleanup](https://github.com/NixOS/nix/blob/209d2bc4428841d4446a3c3b6f75bb5bbfa0f71a/src/libutil/unix/processes.cc).
BSMR additionally requires the kernel's process set to be empty.

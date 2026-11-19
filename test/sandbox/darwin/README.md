<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines Darwin process-lifetime qualification. -->

# Darwin process ownership

A build can exit while a detached child keeps changing its output. This check
requires all processes using one reserved identity to stop before accepting
the output. It exercises the owner in `tools/native`, not a separate test
implementation. It does not yet provide a macOS build executor.

The test refuses root identity, an existing account or group, an occupied
identity, and a lock that is not private and root-owned. It reserves UID and GID
60000 for the duration of the check. Workloads and the signalling helper drop
all root credentials before operating. Each detached writer also has a finite
12-second lifetime, independent of cleanup.

```text
exclusive identity -> detached writer -> launcher exits or is cancelled
                                      -> stop identity -> verify empty
                                                       -> stable output
```

`tools/native` owns the lock and kernel membership check. `identity.rs` drops
the qualification workload's credentials.
`workload.rs` supplies a writer that starts a new session and ignores SIGTERM.
The control must observe continued writes after the launcher exits. The final
check must observe no processes and no further writes after cleanup.

```sh
cargo +1.98.0 test --locked --manifest-path test/sandbox/darwin/Cargo.toml
cargo +1.98.0 build --locked --manifest-path test/sandbox/darwin/Cargo.toml
test/sandbox/darwin/target/debug/bsmr-darwin-check prepare test/sandbox/darwin/target/runtime.txt
sudo test/sandbox/darwin/target/debug/bsmr-darwin-check run /private/var/tmp/bsmr-darwin-check test/sandbox/darwin/target/runtime.txt
```

Use a disposable macOS host. The GitHub-hosted job runs this exact path.
It leaves its inert lock and runtime evidence directory for inspection. A reused
evidence path is rejected. `normal` and `cancel` must both report `empty: true`
and `stable_output: true`.

The filesystem check executes the native binary in a new root with its dynamic
loader, the host OS shared-library cache, and one declared input. After dropping privileges, it must read that input
and fail to read a known host file. It also compiles real Rust metadata with the
pinned toolchain inside this root. This checks compiler loading and source/output
access. It does not qualify linking, procedural macros, or complete consumers.
The cache is copied from the host's OS image into dyld's documented system-cache
directory inside the root. Copying the loader alone fails on modern macOS because
libraries such as `libiconv` no longer exist as separate files.

The child also uses a default-deny native policy with filesystem, process,
system-information, socket creation, and same-sandbox signalling permissions.
Network traffic and Mach service lookup remain denied. Its parent first proves
that the local peer accepts connections. The confined child must receive a
permission error from that same peer. It must also write an output while
failing to overwrite or unlink a root-owned input in the same sticky directory.
These checks separate output permissions from input protection.

The credential-scoped broadcast follows the mechanism in
[Nix's process cleanup](https://github.com/NixOS/nix/blob/209d2bc4428841d4446a3c3b6f75bb5bbfa0f71a/src/libutil/unix/processes.cc#L172).
This check additionally requires the Darwin kernel's UID membership query to
report empty before success. It does not treat successful signal delivery as
proof of termination.

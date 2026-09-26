<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines Darwin process-lifetime qualification. -->

# Darwin process ownership

A build can exit while a detached child keeps changing its output. This check
requires all processes using one reserved identity to stop before accepting
the output. It does not yet provide a macOS build executor or filesystem policy.

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

`identity.rs` owns the lock, credential change and kernel membership check.
`workload.rs` supplies a writer that starts a new session and ignores SIGTERM.
The control must observe continued writes after the launcher exits. The final
check must observe no processes and no further writes after cleanup.

```sh
cargo +1.98.0 test --locked --manifest-path test/sandbox/darwin/Cargo.toml
cargo +1.98.0 build --locked --manifest-path test/sandbox/darwin/Cargo.toml
sudo test/sandbox/darwin/target/debug/bsmr-darwin-check run /private/var/tmp/bsmr-darwin-check
```

Use a disposable macOS host. The GitHub-hosted job runs this exact path.
It leaves its inert lock and small evidence directory for inspection. A reused
evidence path is rejected. `normal` and `cancel` must both report `empty: true`
and `stable_output: true`.

The filesystem check executes the native binary in a new root with its dynamic
loader and one declared input. After dropping privileges, it must read that input
and fail to read a known host file. This tests the native loader boundary before
constructing a compiler runtime. It does not establish network isolation.

The credential-scoped broadcast follows the mechanism in
[Nix's process cleanup](https://github.com/NixOS/nix/blob/209d2bc4428841d4446a3c3b6f75bb5bbfa0f71a/src/libutil/unix/processes.cc#L172).
This check additionally requires the Darwin kernel's UID membership query to
report empty before success. It does not treat successful signal delivery as
proof of termination.

<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Explains the configured Linux sandbox backends and VM execution from macOS. -->

# Sandboxed builds

Use `--sandbox` with an explicitly configured execution backend:

```console
bsmr build <path> --sandbox
bsmr test <path> --sandbox
bsmr run <path> --sandbox
```

## Linux namespaces

The [namespace backend](https://github.com/dedalus-labs/bsmr/blob/main/app/bsmr_execute_impl/src/executors/namespace/README.md)
runs on Linux ARM64 and x86-64. Each action receives a private process tree,
read-only declared inputs, writable outputs and no network access. This backend
supports the native Cargo frontend's build scripts and procedural macros.

Configure the verified runtime inside the Linux worker:

```ini
[sandbox]
backend = namespace
runtime = /opt/bsmr/runtime/runtime.json
```

The runtime must contain the required build tools. Native Rust builds use
Python, a linker, `tar` and `gzip` in addition to the downloaded Rust toolchain.
The runtime manifest pins the actual file bytes that participate in cache identity.

## Linux builds from macOS

Run the Linux BSMR binary and its matching `bsmr-cargo` planner inside an existing
Apple-virtualized Linux VM. Keep the checkout, outputs and cache on the VM's
persistent disk. The namespace backend isolates individual actions inside that VM.

For an existing Apple `container` VM named `build-vm` with a Docker worker named
`bsmr-worker`, an installed Linux binary pair and a staged project, invoke it from macOS:

```console
container exec build-vm docker exec --workdir /work/project bsmr-worker \
  /opt/bsmr/bin/bsmr build app --sandbox --show-output
```

The worker uses a non-root UID and needs a writable `/tmp` directory. A minimal
worker image can provide it with Docker's `--tmpfs /tmp:rw,exec,nosuid,mode=1777`.
Docker must permit its nested namespaces with
`seccomp=unconfined`, `apparmor=unconfined` and `systempaths=unconfined` security
options. The last option permits each action to mount its private `/proc`.
This is the same outer-worker requirement documented for
[rootless BuildKit](https://github.com/moby/buildkit/blob/master/docs/rootless.md).
The action's own namespace policy still restricts process visibility and kernel controls.

This command produces Linux artifacts for the guest architecture. BSMR does not
automatically forward a macOS invocation into a VM or produce native macOS binaries
through this path. Guest setup, source staging and copying final artifacts back to
macOS remain explicit operations.

## Firecracker

The default sandbox backend executes each action in a fresh, networkless
Firecracker microVM.

This backend is experimental and supported only on `x86_64` Linux hosts with KVM
and cgroup v2. It is fail-closed: an incompatible host, action, bundle, or
launcher stops the build instead of running the action on the host.

### Operator setup

An administrator installs the root-owned Firecracker bundle and runs
`bsmr-sandboxd` as a system service. The bundle contains a matched static
Firecracker and jailer release, kernel, and root filesystem. The guest agent is
inside the root filesystem. The manifest pins each of these artifacts by
SHA-256.

Only the privileged launcher needs `/dev/kvm`. It verifies KVM before publishing
its socket; the unprivileged BSMR daemon needs access only to that socket.

The default paths are:

```text
/usr/local/share/bsmr/firecracker/manifest.json
/run/bsmr/sandboxd.sock
```

Override them in the project's existing `.bsmr` file when necessary:

```ini
[sandbox]
bundle = /usr/local/share/bsmr/firecracker/manifest.json
launcher_socket = /run/bsmr/sandboxd.sock
```

The root filesystem must contain the toolchains required by the actions. BSMR
does not download or silently substitute an execution environment. Create the
manifest only after assembling the immutable bundle:

```console
bsmr-sandbox-bundle \
  --directory /usr/local/share/bsmr/firecracker \
  --firecracker-version 1.16.1
```

### v1 contract

The first profile is `untrusted-v1`: one microVM per action, no network device,
2 vCPUs, 2 GiB of memory, explicit environment variables, declared inputs only,
declared outputs only, and complete VM teardown before the result is accepted.

Persistent workers, inherited host environments, absolute executables,
incremental output state, required local resources, detached processes,
secrets, custom devices, snapshots, VM reuse, and remote execution are not
supported by this profile. BSMR reports these as compatibility errors.

See the [implementation design](https://github.com/dedalus-labs/bsmr/blob/main/docs/developers/firecracker-sandbox.md)
for the threat model, protocol, exemplar audit, and conformance gates.

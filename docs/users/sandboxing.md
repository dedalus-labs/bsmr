<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Explains the supported local Firecracker sandbox and its operator contract. -->

# Sandboxed builds

Use `--sandbox` to execute each action in a fresh, networkless Firecracker
microVM restored from an immutable pre-action snapshot:

```console
bsmr build <path> --sandbox
bsmr test <path> --sandbox
bsmr run <path> --sandbox
```

Sandboxing is experimental and supported only on `x86_64` Linux hosts with a
Kernel-based Virtual Machine (KVM)
and cgroup v2. It is fail-closed: an incompatible host, action, bundle, or
launcher stops the build instead of running the action on the host.

## Operator setup

An administrator installs the root-owned Firecracker bundle and runs
`bsmr-sandboxd` as a system service. The bundle contains a matched static
Firecracker and jailer release, kernel, and root filesystem. The bundle command
boots the guest to its pre-action barrier and adds the resulting VM state and
memory image. The manifest pins every artifact by SHA-256.

Only the privileged launcher needs `/dev/kvm`. It verifies KVM before publishing
its socket; the unprivileged BSMR daemon needs access only to that socket.

The default paths are:

```text
/usr/local/share/bsmr/firecracker/manifest.json
/run/bsmr/sandboxd.sock
/var/cache/bsmr/cas
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

The bundle, local content-addressed store (CAS), and jail root must share one
filesystem so immutable
objects can be hard-linked without copying. BSMR fails at launcher startup when
this zero-copy contract cannot be met.

## Cache behavior

BSMR calculates the complete sandboxed action key before it contacts the
launcher. It checks the local cache when one is configured. When an execution
platform enables remote caching, BSMR checks its Remote Execution API action
cache and restores missing output bytes from content-addressed storage (CAS).

A valid cache hit materializes the declared outputs without starting
Firecracker. A miss runs the action in a microVM and uploads a successful result
only when cache uploads are enabled. The action key includes the sandbox
profile, protocol, backend, and complete bundle digest, so another kernel,
root filesystem, guest agent, Firecracker binary, or snapshot cannot reuse the
result.

Remote caching does not enable remote execution. CI controls cache credentials
and write access; untrusted pull requests should use a read-only public cache
namespace or no remote cache.

## v1 contract

The first profile is `untrusted-v1`: one pristine snapshot clone per action, no
network device, 2 vCPUs, 2 GiB of memory, explicit environment variables,
declared inputs only, declared outputs only, fresh guest entropy, and complete
VM teardown before the result is accepted.

Persistent workers, inherited host environments, absolute executables,
incremental output state, required local resources, detached processes,
secrets, custom devices, post-action VM reuse, and remote execution are not
supported by this profile. BSMR reports these as compatibility errors.

See the [implementation design](https://github.com/dedalus-labs/bsmr/blob/main/docs/developers/firecracker-sandbox.md)
for the threat model, protocol, exemplar audit, and conformance gates.

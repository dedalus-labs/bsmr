<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines the local Firecracker sandbox's security and execution contract. -->

# Local Firecracker sandbox

BSMR can run each declared action in a new, networkless Firecracker microVM.
The launcher restores each VM from one authenticated, pristine snapshot that
contains no action state.

The `untrusted-v1` profile is available through `--sandbox` on `x86_64` Linux
hosts with Kernel-based Virtual Machine (KVM) support.

Use the profile with build, test, or run commands:

```console
bsmr build <path> --sandbox
bsmr test <path> --sandbox
bsmr run <path> --sandbox
```

The command fails before execution if the host, action, bundle, or privileged
launcher cannot satisfy the profile. BSMR does not retry the action through the
ordinary host executor.

## Execution contract

Each action receives the same fixed environment contract.

| Property | `untrusted-v1` value | Consequence |
| --- | --- | --- |
| VM lifetime | One pristine snapshot clone per action. | Repository code never shares a guest with another action. |
| Virtual CPUs | 2 virtual CPUs (vCPUs). | The launcher rejects a request for another machine shape. |
| Memory | 2 gibibytes (GiB). | The launcher rejects a request for another memory size. |
| Network | No emulated network device. | The action cannot use a network policy other than `none`. |
| Inputs | One read-only declared-input archive. | The guest cannot read the host checkout or undeclared files. |
| Outputs | One private writable output volume. | The host imports only validated, declared outputs. |
| Environment | The declared action environment only. | The action cannot inherit host environment variables. |
| Firecracker process | Same-version Firecracker and jailer binaries. | The launcher rejects a mismatched or mutable bundle. |
| Snapshot | One paused pre-action VM state and memory image. | A VM that runs repository code can never become a snapshot source. |
| Entropy | Fresh kernel entropy before action state is read. | Two clones do not start with the same kernel random state. |
| Terminal state | Complete VM cleanup before response. | Guest exit alone never proves that host resources are gone. |

Protocol v2 does not support post-action VM reuse, persistent workers, networked
actions, secrets, custom devices, or remote execution. Each feature changes the
security or cache contract and requires separate conformance evidence.

## System boundary

Firecracker is the virtual machine monitor (VMM). It supplies the KVM boundary
and a small emulated device model. It does not construct the action filesystem,
authorize launcher clients, constrain its host process, or validate outputs.

A Linux control group (cgroup) limits and owns a process tree. A virtio block
device presents a host file to the guest as a virtual disk.

BSMR treats the following processes as one executor:

```text
unprivileged BSMR daemon
    |
    | authenticated request plus three file descriptors
    v
privileged bsmr-sandboxd launcher
    |
    | same-version jailer, private cgroup, immutable bundle
    v
unprivileged Firecracker process with private snapshot memory
    |
    | KVM, four virtio block devices, private vsock, and virtio-rng
    v
BSMR guest agent as process ID 1
    |
    | declared command, environment, inputs, and outputs
    v
action process
```

The launcher runs with privileges because the official jailer must create the
changed root directory (chroot), namespaces, cgroups, and device nodes before
it drops privileges. A build must not invoke `sudo`, install a set-user-ID
executable, or run Firecracker without the launcher.

Only the launcher needs `/dev/kvm`. The launcher verifies KVM before it
publishes the Unix socket. The unprivileged BSMR daemon needs permission to
connect to that socket, but it does not need KVM access.

The virtual socket (vsock) carries one private wake byte from the launcher to
the restored guest. The virtio random-number device (`virtio-rng`) supplies
fresh host entropy after each restore.

## Request flow

The executor processes one action in this order:

1. The BSMR daemon validates that the action uses no inherited environment,
   persistent worker, network, local resource, incremental output, or detached
   process behavior.
2. The daemon decodes the canonical Remote Execution `Action` and `Command`.
3. The daemon reads every declared input and verifies its analyzed digest.
4. The daemon writes the inputs to a deterministic tar archive, then calculates
   SHA-256 digests for the action and input transport files.
5. The daemon sends the request and three file descriptors to
   `bsmr-sandboxd` over a group-restricted Unix socket.
6. The launcher validates the request, descriptor types, access modes, sizes,
   content digests, machine shape, timeout, and execution-bundle digest.
7. The launcher creates a unique user ID, group ID, jail, process ID namespace,
   and cgroup for the action.
8. The launcher hard-links the immutable kernel, root filesystem, VM state, and
   memory image from its local content-addressed store into the jail.
9. The launcher copies the authenticated action and input bytes into private,
   fixed-capacity block files.
10. The jailer creates `/dev/kvm`, enters the jail, drops privileges, and starts
   Firecracker.
11. Firecracker maps the pristine memory image with private copy-on-write pages
    and resumes the paused VM.
12. The launcher connects to the guest's pre-action vsock listener, waits for a
    guest readiness byte after any restore-time transport reset, and then sends
    one action-release byte.
13. The guest mixes 256 fresh bits from `virtio-rng` into the kernel random
    pool before it reads the action request.
14. The guest mounts a private temporary filesystem at `/workspace`, unpacks
    the declared inputs, and starts the action as user 1000.
15. The guest writes the result envelope, standard output, standard error, and
    declared output trees to the output volume.
16. The launcher terminates the complete VM boundary. The daemon then validates
    and imports the output archive.

No step can select a host-execution fallback. A failure returns an error from
the current boundary and leaves the action unexecuted or failed.

## Threat model

BSMR treats the action, its inputs, and every process that it starts as
malicious. Action code might try to read host files, contact a service, consume
unbounded resources, forge output metadata, survive cancellation, or exploit a
trusted component.

The trusted computing base contains:

- the BSMR daemon's action construction and output validator;
- the launcher's Unix-socket authorization and VM lifecycle code;
- the exact Firecracker and jailer binaries;
- the exact guest kernel and root filesystem;
- the BSMR guest agent inside that root filesystem;
- KVM and the host kernel; and
- the administrator who installs the bundle and launcher policy.

The profile protects the host and concurrent BSMR actions from repository code.
It does not protect against a malicious host administrator, host-kernel
compromise, hardware side channels, or a vulnerability in the trusted computing
base.

Restoring one snapshot more than once can duplicate random state and identifiers.
Firecracker changes the Virtual Machine Generation Identifier (VMGenID) on
restore, which causes supported Linux kernels to reseed their random pool. The
guest also reads 256 fresh bits from `virtio-rng` before it reads action state.

The pristine guest image must not contain a precomputed token, cached random
number, userspace pseudorandom number generator, or action-specific identifier.

## Action and environment identity

The Remote Execution `Action` and `Command` remain the execution application
binary interface (ABI). BSMR adds four sorted platform properties before it
calculates the action digest:

```text
bsmr.sandbox.profile = untrusted-v1
bsmr.sandbox.backend = firecracker
bsmr.sandbox.environment = sha256:<bundle-manifest-digest>
bsmr.sandbox.protocol = 2
```

The bundle manifest records the architecture and SHA-256 digest of Firecracker,
the jailer, the guest kernel, the read-only root filesystem, the paused VM state,
and the pristine memory image. The root filesystem contains the guest agent.

The manifest also records a fingerprint of the snapshot source's CPU features,
microcode, and host-kernel release. A change to any artifact or host fingerprint
changes the environment digest and therefore changes the action key.

A file path, mutable tag, or version string is not an environment identity.
The launcher verifies artifact bytes and also verifies that the Firecracker and
jailer executables report the manifest's release.

## Input filesystem

The guest sees only declared action inputs. The host does not mount the checkout,
package store, home directory, or host temporary directory into the microVM.

For example, an action can declare these inputs and output:

```text
inputs:
  package.json
  src/main.ts

output:
  dist/
```

The daemon creates an archive that contains `package.json` and `src/main.ts`.
The guest unpacks those files beneath `/workspace`. If the command reads
`../secrets.txt`, the file does not exist in the guest because it was not a
declared input.

The input encoder enforces these rules:

- Each path must be normalized and project-relative.
- Each regular file must match the digest recorded during analysis.
- Each symbolic link must remain inside the action root.
- External symbolic links are invalid.
- The archive must remain within its byte, node, and path-depth limits.

The launcher exposes the archive as a read-only virtio block device. The guest
unpacks it into a new temporary filesystem for each action.

The launcher copies the exact admitted byte range with Linux `sendfile(2)` and
verifies its SHA-256 digest before it extends the private block file to its fixed
capacity. A same-size descriptor mutation therefore fails before VM startup.

## Command environment

The guest starts the command in the declared Remote Execution working directory.
It clears the guest process environment, applies the declared variables, and
sets `TMPDIR` to a private directory under `/workspace`.

The command can use a project-relative executable or an unqualified executable
that the guest resolves through its declared `PATH`. Protocol v1 rejects an
absolute executable because it would couple the action to an undeclared guest
filesystem path.

## Output validation

The guest output archive is untrusted. The daemon validates the complete archive
before it writes any project output.

The archive can contain only:

- one versioned result envelope;
- bounded standard output and standard error files; and
- declared regular files, directories, and relative symbolic links.

The validator enforces these rules:

- Every path must be normalized, relative, unique, and declared.
- A file output cannot contain descendants.
- A symbolic link must remain inside its declared output root.
- Hard links and special file types are invalid.
- File bytes, node count, path depth, standard output, and standard error must
  remain within their limits.
- A declared file and a declared directory cannot overlap.

After validation, the daemon rewinds the archive and materializes it into an
empty staging directory. A validation failure deletes the staging directory.
An existing output destination is an error. The executor does not delete or
overwrite an existing destination to make an import succeed.

The ordinary BSMR materializer hashes accepted outputs into the
content-addressed store (CAS).

## Process ownership and cleanup

The launcher assigns each active VM a unique user ID, group ID, jail, and
cgroup. The action identifier is a random lowercase universally unique
identifier (UUID), so two executions of the same action cannot select the same
jail path.

At launcher startup, the launcher hard-links each verified kernel, root
filesystem, VM-state, and memory artifact beneath its SHA-256 name in a local
CAS. The bundle, CAS, and jail root must share one filesystem because a hard
link cannot cross a filesystem boundary.

Each action jail hard-links those immutable objects again. Firecracker maps the
shared memory image with `MAP_PRIVATE`, the Linux private memory-mapping mode.
Unchanged pages remain shared through the host page cache. A guest write creates
an anonymous copy-on-write page and does not change the source memory file.

The snapshot source stops at the guest listener before it reads an action or
input byte. The bundle builder pauses that VM, writes the state and memory
artifacts, and terminates the source process. It never resumes the source VM.

Firecracker resets vsock during snapshot restoration. The launcher may replace
a connection only before the guest reports readiness and before the launcher
sends the action-release byte. Once release starts, an I/O failure fails the
action; the launcher never retries an ambiguous release.

The launcher applies a parent-death signal to the supervisor. It also applies
an outer action deadline. Cancellation or daemon disconnect closes the client
socket, which tells the launcher to stop the VM.

Before the launcher returns a terminal response, it must:

1. kill the complete action cgroup;
2. wait for the supervisor and Firecracker processes to exit;
3. verify that the recorded Firecracker process ID no longer exists;
4. remove the action cgroup; and
5. remove the action jail.

If any cleanup step fails, the response sets `cleanup_complete` to false. The
daemon rejects that response even if the guest reported successful execution.

## Host and bundle requirements

The launcher must reject work unless all of these conditions are true:

- The host runs `x86_64` Linux.
- `/dev/kvm` is an accessible character device.
- The host uses cgroup v2 with the memory and process-count controllers.
- The Firecracker and jailer files are static musl executables from the same
  release.
- Every bundle artifact matches its manifest digest and architecture.
- The source and target CPU features, microcode, and host-kernel release have
  the same fingerprint.
- The bundle, local CAS, and jail root share one filesystem.
- Root owns the bundle and its path chain.
- Action identities cannot write the bundle, launcher path, jail root, or Unix
  socket directory.
- The configured user ID and group ID range does not overlap a host account.
- The jail root and Unix socket use safe ownership and modes.

macOS, Windows, and non-`x86_64` Linux hosts return a typed unsupported-host
error. They do not select another VMM under `--sandbox`.

## Failure behavior

| Failure | Detection boundary | Result |
| --- | --- | --- |
| Unsupported host or missing cgroup v2 | BSMR daemon and launcher startup. | The command fails before action execution. |
| Missing or invalid `/dev/kvm` | Launcher startup. | The launcher does not publish its socket. |
| Bundle digest or ownership mismatch | Daemon or launcher startup. | The command fails before action execution. |
| Snapshot host fingerprint mismatch | Launcher startup. | The launcher does not publish its socket. |
| Bundle and jail filesystems differ | Launcher startup. | The launcher does not publish its socket. |
| Unsupported action semantics | Daemon policy validation. | The action does not run in the VM or on the host. |
| Input mutation after analysis | Input archive construction. | The VM does not start. |
| Guest listener does not reach the pre-action barrier | Launcher startup. | The action fails before the guest reads action state. |
| Client cancellation or disconnect | Launcher socket monitoring. | The launcher kills and cleans the VM. |
| Guest timeout | Guest deadline and launcher deadline. | The action reports a timeout after cleanup. |
| Invalid or undeclared output | Host output validation. | The daemon discards staging and fails the action. |
| Incomplete cgroup or jail cleanup | Launcher response validation. | The daemon rejects the result. |

## Conformance requirements

A release must run the real Firecracker path on an `x86_64` Linux runner with
nested KVM. A skipped KVM test is not a passing sandbox test.

The conformance corpus must prove these properties:

1. The action key changes with the profile, protocol, and every bundle
   artifact.
2. Unsupported hosts, inaccessible KVM, mismatched architecture, bad digests,
   unsafe ownership, missing launchers, and mismatched executables fail before
   action execution.
3. Absolute reads, parent traversal, symbolic-link traversal, host writes,
   inherited environments, host sockets, and network access fail inside a real
   microVM.
4. Declared files, directories, executable bits, Unicode names, empty trees,
   relative symbolic links, standard streams, nonzero exit codes, and timeouts
   survive the round trip.
5. Undeclared, absolute, escaping, special, oversized, deep, and hard-linked
   outputs fail before materialization.
6. Cancellation and guest hangs leave no process, cgroup, mount, jail, socket,
   or writable image behind.
7. Concurrent identical actions cannot observe or overwrite each other.
8. A second clean run produces the same output digest.
9. `--sandbox` never invokes the ordinary host executor.
10. Successful, timed-out, and canceled actions complete launcher cleanup.
11. Fresh boot and snapshot restore pass the same action corpus.
12. Two clones observe different kernel random bytes before action code runs.

Unit tests use a fake launcher only at the protocol boundary. They complement
the KVM test but do not replace it.

## Performance evidence

CI compares snapshot restore with a fresh boot on the same nested-KVM runner.
Both modes run the same conformance action before the benchmark starts.

Each mode records 30 environment-start samples and 30 complete client-roundtrip
samples. Environment-start time ends after the launcher releases the guest's
pre-action barrier. Roundtrip time includes transport preparation, action
execution, output extraction, and verified cleanup.

CI reports the 50th, 95th, and 99th percentiles (p50, p95, and p99). Snapshot
restore must be at least four times faster than fresh boot at both p50 and p95
for environment-start time.

The machine-readable artifact retains all raw samples, the bundle environment
digest, and the host fingerprint. The gate does not generalize this result to
another host, bundle, action fixture, or workload.

## Future work

Protocol v2 does not use a userfaultfd memory pager, asynchronous block engine,
network device, remote direct memory access (RDMA) path, post-action VM pool, or
remote executor. These mechanisms add contracts that the current profile does
not test.

Future work can add a mechanism only when the action identity binds its complete
state and the existing conformance corpus cannot distinguish it from the
current implementation.

## Reference implementations

| Concern | Reference | Rule retained by BSMR |
| --- | --- | --- |
| Undeclared reads and writes | [Bazel hermetic sandbox tests](https://github.com/bazelbuild/bazel/blob/f644f2dff90cbbba0e14551051578c5e93328650/src/test/shell/bazel/bazel_hermetic_sandboxing_test.sh#L296-L355) | Build the workspace from declared inputs and import only declared outputs. |
| Namespace and timeout handling | [Bazel Linux sandbox](https://github.com/bazelbuild/bazel/blob/f644f2dff90cbbba0e14551051578c5e93328650/src/main/tools/linux-sandbox.cc#L139-L217) | Own descendants and verify cleanup before reporting a result. |
| VMM containment | [Firecracker threat containment](https://github.com/firecracker-microvm/firecracker/blob/48f1b9fb52e90f00b61adefcad002183d07195c1/docs/design.md#L81-L95) and [process sandboxing](https://github.com/firecracker-microvm/firecracker/blob/48f1b9fb52e90f00b61adefcad002183d07195c1/docs/design.md#L152-L194) | Run Firecracker through a same-version jailer. |
| Privileged launcher | [Firecracker jailer operations](https://github.com/firecracker-microvm/firecracker/blob/48f1b9fb52e90f00b61adefcad002183d07195c1/docs/jailer.md#L109-L161) | Keep device, namespace, cgroup, chroot, and privilege setup outside the build. |
| Host and guest control | [firecracker-containerd architecture](https://github.com/firecracker-microvm/firecracker-containerd/blob/be68640a5d2237f5b427c37c1f5809ec154126c5/docs/architecture.md#L24-L42) | Keep lifecycle control on the host and command execution in a guest agent. |
| Block storage | [firecracker-containerd snapshotters](https://github.com/firecracker-microvm/firecracker-containerd/blob/be68640a5d2237f5b427c37c1f5809ec154126c5/docs/snapshotter.md#L1-L43) | Use private block devices because Firecracker does not share a host filesystem. |
| Snapshot uniqueness | [Firecracker snapshot security](https://github.com/firecracker-microvm/firecracker/blob/48f1b9fb52e90f00b61adefcad002183d07195c1/docs/snapshotting/snapshot-support.md#snapshot-security-and-uniqueness) | Never clone action state without a mechanism that restores uniqueness. |
| Fast isolated compute | [Aurora DSQL query-processor isolation](https://arxiv.org/pdf/2607.13276v2#page=5) | Treat prebooted immutable state as a possible optimization, not as proof of action isolation. |

The CI implementation also depends on the [Firecracker KVM initialization
gate](https://github.com/firecracker-microvm/firecracker/blob/48f1b9fb52e90f00b61adefcad002183d07195c1/tools/devtool#L750-L854)
and the [Blacksmith nested-virtualization
contract](https://docs.blacksmith.sh/blacksmith-runners/overview#faq).

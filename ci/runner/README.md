<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines approved build routing, child ownership, and fleet prerequisites. -->

# Mac build routing

The `Build Rust on Mac` workflow builds one administrator-approved source SHA.
Its controller tries Dedalus Machines, Blacksmith, then GitHub capacity.
Each route runs the same macOS ARM64 compiler and qualification commands.

```text
main workflow + administrator + exact source
  -> one provider dispatch
     -> verify parent ownership
     -> build the approved source
  -> completed result or confirmed unassigned cancellation
```

| Interface | Responsibility |
| --- | --- |
| `build.ts` | Generate the hosted controller, authorization job, and native payload. |
| `action.ts` | Expose authorization, dispatch, wait, cleanup, and completion operations. |
| `api.ts` | Validate GitHub records and bind calls to this repository. |
| `lifecycle.ts` | Preserve one attempt's ownership until a terminal result is known. |

Manual dispatch requires a full commit SHA and fresh repository administrator
permission. The workflow definition must come from `main`. Child dispatches
also verify the parent's unfinished state, definition SHA, source identity,
and triggering administrator. The run title records the source approved by
the immutable workflow inputs. A definition change during dispatch fails.

The controller keeps the run ID returned by GitHub. After 60 seconds without
assignment on office or Blacksmith capacity, it cancels that attempt and waits
up to 30 seconds for terminal proof. Only a complete job inventory proving that
the payload never received a runner permits handoff. GitHub's final queue gets
300 seconds. A started, failed, or ambiguously cancelled build is not replayed.
Separate cleanup steps retain child IDs when the parent is cancelled.

Payloads recheck ownership after leaving the queue and before checking out the
approved source. They receive read-only repository and Actions access. Dispatch credentials stay in
GitHub-hosted control jobs. The build uses two Cargo jobs, disables development
debug data and incremental compiler state for building BSMR itself, and then
runs the native qualification harnesses. This does not alter consumer profiles.

## Fleet prerequisite

Before enabling office routing, restrict the runner group to this repository
and the workflow `.github/workflows/runner-build.yml@refs/heads/main`. Register
the standard `self-hosted`, `macOS`, and `ARM64` labels plus `dedalus-machines`.
Preserve the fleet's one-job admission and checkout/HOME cleanup hooks. Shared
native accounts are not VM isolation. Fork workflows must not reach these hosts.

Workflow publication does not enroll hosts or prove capacity. Verify actual
runner permissions, memory/disk availability, pinned tools, successful builds,
busy-pool handoff, cancellation, and cleanup before automatic CI adoption.
The existing Linux and release workflows retain their own platform contracts.

## Verification

```console
node --test ci/runner/*.test.ts
pnpm run ci check
```

Tests cover authorization, exact source/definition ownership, lost dispatch
responses, incomplete inventories, queue saturation, assignment during
cancellation, real payload results, and cleanup. These tests simulate GitHub's
external state. Connected runner receipts are a separate rollout requirement.

See [GitHub runner groups](https://docs.github.com/en/actions/how-tos/manage-runners/self-hosted-runners/manage-access)
and [hosted Mac specifications](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).

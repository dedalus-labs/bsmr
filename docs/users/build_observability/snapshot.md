<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines experimental file snapshots and their verification boundary. -->

# Experimental build snapshot

`bsmr build --snapshot PATH` captures successful file outputs without changing
build inputs or cache keys. It reads `bsmr.component.toml` in the command's
working directory and creates a new TOML lock. It never replaces an existing
file. The output's parent directory must exist. Snapshot capture cannot be combined
with build-report output flags.

```toml
schema = 1
name = "hello"

[provides]
"hello.http" = 1
```

```sh
bsmr build //:bin //worker:bin --snapshot snapshots/bsmr.lock
```

The lock uses `bsmr.dependency-lock.experimental.v0`, not RFC 0004's final schema.
It contains a canonical declaration digest, the declaration, configured targets,
project-relative output paths, file CAS digests (including sizes), executable
bits, and available command action digests. Inline copies and source outputs
have no command digest and omit that optional field. It omits run IDs, timings, and checkout paths.
It reads typed build results using the metadata from PR #221. Only default
outputs are captured. Additional runtime files are not captured yet.

Failed or skipped targets, empty results, non-file outputs, unknown manifest fields, and external requirements reject.
Readers must reject unknown schemas. This prototype has one local component.

This is a build-output snapshot, not a complete source/dependency lock or a
release receipt. It does not prove source review, hermetic execution, test success,
remote-cache trust, or durable publication. No production consumer should accept
it as certification. Normal builds do not read or mutate this file yet.

TODO(builder): capture reviewed source trees and exact external input locks.
TODO(cache): qualify cold-runner remote restoration using existing cache clients.
TODO(resolver): verify RFC 0004 contract ranges before accepting external components.
TODO(publisher): retain artifacts and lock durably with a separate authenticated CI receipt.

These are integration points for existing machinery, not new generic traits.

Run `python3 test/build-snapshot.py /path/to/bsmr` with Python 3.11 or newer.
It compiles and runs two Go executables, checks snapshot hashes, edits one
service, checks that the worker does not execute again, restores the original
snapshot, and rejects overwrites and failed builds.
